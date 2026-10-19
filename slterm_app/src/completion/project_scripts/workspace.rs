//! 有界 workspace 声明发现与 scripts 交集计算。
//!
//! 只读声明文件(package.json / pnpm-workspace.yaml):不导入、不执行项目
//! 代码,也不调用包管理器。总预算 500ms + 4MB + 4096 目录 + 512 包;任一
//! 超限即放弃该来源(候选缺失优于猜测)。

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slterm_completions::semantic::{Context, PackageManager, Source};

struct Package {
    name: String,
    path: String,
    scripts: BTreeSet<String>,
    dependencies: BTreeSet<String>,
}

struct Discovery<'a> {
    cancelled: &'a dyn Fn() -> bool,
    deadline: Instant,
    bytes: usize,
    directories: usize,
}

impl Discovery<'_> {
    fn ready(&self) -> bool {
        !(self.cancelled)() && Instant::now() < self.deadline
    }

    fn read(&mut self, path: &Path, limit: usize) -> Option<Vec<u8>> {
        if !self.ready() {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file()
            || metadata.len() as usize > limit
            || self.bytes + metadata.len() as usize > 4 * 1024 * 1024
        {
            return None;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .ok()?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        self.bytes += bytes.len();
        (bytes.len() <= limit && self.bytes <= 4 * 1024 * 1024 && self.ready()).then_some(bytes)
    }

    fn manifest(&mut self, path: &Path) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.read(path, 1024 * 1024)?).ok()
    }
}

fn package(json: &serde_json::Value, path: String) -> Package {
    let scripts = json
        .get("scripts")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(name, value)| !name.starts_with('-') && value.is_string())
        .map(|(name, _)| name.clone())
        .collect();
    let dependencies = ["dependencies", "devDependencies", "optionalDependencies"]
        .into_iter()
        .flat_map(|key| {
            json.get(key)
                .and_then(serde_json::Value::as_object)
                .into_iter()
                .flatten()
        })
        .map(|(name, _)| name.clone())
        .collect();
    Package {
        name: json
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&path)
            .to_owned(),
        path,
        scripts,
        dependencies,
    }
}

pub(super) fn read(
    cwd: &Path,
    context: &Context,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<String>> {
    let mut discovery = Discovery {
        cancelled,
        deadline: Instant::now() + Duration::from_millis(500),
        bytes: 0,
        directories: 0,
    };
    let (root, json, yaml) = find_root(cwd, context, &mut discovery)?;
    let patterns = patterns(&json, yaml.as_deref(), context.project.manager)?;
    let mut packages = Vec::new();
    collect(&root, &root, &patterns, &mut packages, &mut discovery)?;
    if context.project.include_root
        || context.project.manager == PackageManager::Pnpm && yaml.is_none()
    {
        packages.push(package(&json, ".".into()));
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    packages.dedup_by(|a, b| a.path == b.path);
    finish(&packages, context)
}

fn find_root(
    cwd: &Path,
    context: &Context,
    discovery: &mut Discovery<'_>,
) -> Option<(PathBuf, serde_json::Value, Option<String>)> {
    let mut root = context
        .directories
        .last()
        .map_or_else(|| cwd.to_owned(), |dir| cwd.join(dir));
    let mut nearest = None;
    for _ in 0..64 {
        if !discovery.ready() || !root.is_dir() {
            return None;
        }
        let manifest = root.join("package.json");
        if manifest.exists() {
            let json = discovery.manifest(&manifest)?;
            let pnpm = root.join("pnpm-workspace.yaml");
            let yaml = if pnpm.exists() {
                Some(String::from_utf8(discovery.read(&pnpm, 64 * 1024)?).ok()?)
            } else {
                None
            };
            if context.project.manager == PackageManager::Pnpm {
                if yaml.is_some() {
                    return Some((root, json, yaml));
                }
                if nearest.is_none() {
                    nearest = Some((root.clone(), json, None));
                }
            } else if !patterns(&json, yaml.as_deref(), context.project.manager)?.is_empty() {
                return Some((root, json, yaml));
            }
        }
        if root.join("node_modules").is_dir() || !root.pop() {
            return nearest;
        }
    }
    None
}

pub(in crate::completion) fn finish_catalog(
    catalog: &serde_json::Value,
    context: &Context,
) -> Option<Vec<String>> {
    let packages: Vec<_> = catalog
        .as_array()?
        .iter()
        .take(513)
        .map(|entry| {
            Some(package(
                entry.get("manifest")?,
                entry.get("path")?.as_str()?.to_owned(),
            ))
        })
        .collect::<Option<_>>()?;
    if packages.len() > 512 {
        return None;
    }
    finish(&packages, context)
}

fn finish(packages: &[Package], context: &Context) -> Option<Vec<String>> {
    if matches!(context.source, Source::Workspaces) {
        let path_names = context.value_prefix().starts_with('.')
            || context.value_prefix().contains('/') && !context.value_prefix().starts_with('@');
        return Some(
            packages
                .iter()
                .map(|package| {
                    if path_names && context.project.manager != PackageManager::Yarn {
                        if context.value_prefix().starts_with("./") {
                            format!("./{}", package.path)
                        } else {
                            package.path.clone()
                        }
                    } else {
                        package.name.clone()
                    }
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        );
    }
    let chosen = select(packages, context)?;
    if chosen.is_empty() {
        return Some(Vec::new());
    }
    let mut scripts: Option<BTreeSet<String>> = None;
    for index in chosen {
        let next = &packages[index].scripts;
        scripts = Some(match scripts {
            None => next.clone(),
            Some(previous)
                if context.project.allow_missing
                    || context.project.manager == PackageManager::Pnpm =>
            {
                previous.union(next).cloned().collect()
            }
            Some(previous) => previous.intersection(next).cloned().collect(),
        });
    }
    Some(scripts.unwrap_or_default().into_iter().collect())
}

pub(in crate::completion) fn patterns(
    json: &serde_json::Value,
    yaml: Option<&str>,
    manager: PackageManager,
) -> Option<Vec<String>> {
    if manager == PackageManager::Pnpm {
        return match yaml {
            Some(yaml) => parse_pnpm_packages(yaml),
            None => Some(vec!["**".into()]),
        };
    }
    Some(
        json.get("workspaces")
            .map(|value| value.get("packages").unwrap_or(value))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

/// 有界窄解析 `pnpm-workspace.yaml` 的 `packages` 字符串数组。
///
/// 只覆盖 pnpm workspace 声明实际用到的 YAML 子集:
/// - `packages:` 块序列(`- pattern`,可带单/双引号与行尾注释);
/// - `packages: [a, b]` 行内数组(简单逗号拆分 + 引号);
/// - 空行、`#` 注释与其它顶层键(含其嵌套内容)忽略;
/// - `packages` 键缺失 → pnpm 默认 `**`;空行内数组 → 空列表。
///
/// 锚点/别名/多文档/嵌套流集合等高级特性不实现(`---`/`...` 忽略);
/// 未闭合引号、`packages` 值不是序列、重复键等明显非法输入返回 None
/// (宁可放弃来源,不猜测)。
fn parse_pnpm_packages(text: &str) -> Option<Vec<String>> {
    let mut packages: Option<Vec<String>> = None;
    let mut in_packages = false;
    let mut saw_packages = false;
    for raw in text.lines() {
        let line = strip_yaml_comment(raw);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "---" | "...") {
            in_packages = false;
            continue;
        }
        // 缩进以原始行首空白判定(注释剥离后的尾部空白不算缩进)。
        let indented = raw.starts_with([' ', '\t']);

        if in_packages {
            if let Some(item) = block_sequence_item(trimmed) {
                let item = item.trim();
                // `-` 后面为空 = null 项;旧实现 filter_map(as_str) 会过滤掉。
                if !item.is_empty() {
                    packages
                        .get_or_insert_with(Vec::new)
                        .push(parse_yaml_scalar(item)?);
                }
                continue;
            }
            if indented {
                // packages 块内的缩进非序列内容(映射等)= 不是字符串序列。
                return None;
            }
            // 顶层其它键结束 packages 块,继续按顶层键处理。
        } else if indented {
            // 其它顶层键的嵌套内容,忽略。
            continue;
        }

        // 顶层键:只认 `packages:`(允许 `packages :`,允许前导空白)。
        in_packages = false;
        let Some(rest) = trimmed.strip_prefix("packages") else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix(':') else {
            // `packagesList:` 等其它前缀键。
            continue;
        };
        if saw_packages {
            // serde_yaml 对重复映射键报错。
            return None;
        }
        saw_packages = true;
        let value = value.trim();
        if value.is_empty() {
            in_packages = true;
        } else if value.starts_with('[') {
            packages = Some(parse_inline_packages(value)?);
        } else {
            // 标量/null/映射值都不是序列。
            return None;
        }
    }
    if !saw_packages {
        // pnpm 缺省:全量匹配。
        return Some(vec!["**".to_owned()]);
    }
    // `packages:` 后没有任何序列项 = null,旧实现的 as_sequence()? 返回 None。
    packages
}

/// 去掉行尾注释:引号外且位于行首/空白之后的 `#` 才是注释起点。
fn strip_yaml_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    let mut after_whitespace = true;
    for (index, ch) in line.char_indices() {
        match quote {
            Some(active) => {
                if active == '"' && escaped {
                    escaped = false;
                } else if active == '"' && ch == '\\' {
                    escaped = true;
                } else if ch == active {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '#' if after_whitespace => return &line[..index],
                _ => {}
            },
        }
        after_whitespace = ch.is_whitespace();
    }
    line
}

/// YAML 块序列标记:`-` 后必须是空白或行尾;`-foo` 是裸标量。
fn block_sequence_item(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('-')?;
    (rest.is_empty() || rest.starts_with([' ', '\t'])).then_some(rest)
}

/// 解析单个标量:单/双引号包裹或裸值;引号后只允许空白。
fn parse_yaml_scalar(value: &str) -> Option<String> {
    let value = value.trim();
    let quote = value.chars().next()?;
    if quote == '\'' || quote == '"' {
        let (scalar, rest) = take_quoted_scalar(value)?;
        if !rest.trim().is_empty() {
            return None;
        }
        return Some(scalar);
    }
    Some(value.to_owned())
}

/// 取引号标量内容与剩余串;未闭合返回 None。
fn take_quoted_scalar(value: &str) -> Option<(String, &str)> {
    let mut chars = value.char_indices();
    let (_, quote) = chars.next()?;
    let mut scalar = String::new();
    let mut escaped = false;
    while let Some((index, ch)) = chars.next() {
        if quote == '"' {
            if escaped {
                scalar.push(match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return Some((scalar, &value[index + 1..]));
            }
        } else if ch == '\'' {
            let rest = &value[index + 1..];
            if rest.starts_with('\'') {
                // YAML 单引号内 `''` 表示一个 `'`。
                scalar.push('\'');
                chars.next();
                continue;
            }
            return Some((scalar, rest));
        }
        scalar.push(ch);
    }
    None
}

/// 解析 `[a, 'b', "c"]` 行内数组;尾逗号允许,空项与未闭合引号拒绝。
fn parse_inline_packages(value: &str) -> Option<Vec<String>> {
    let body = value.strip_prefix('[')?;
    let mut items: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut closed = false;
    for (index, ch) in body.char_indices() {
        match quote {
            Some(active) => {
                if active == '"' && escaped {
                    escaped = false;
                } else if active == '"' && ch == '\\' {
                    escaped = true;
                } else if ch == active {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                ',' => {
                    items.push(&body[start..index]);
                    start = index + ch.len_utf8();
                }
                ']' => {
                    items.push(&body[start..index]);
                    if !body[index + 1..].trim().is_empty() {
                        return None;
                    }
                    closed = true;
                    break;
                }
                // 嵌套流集合不在窄解析范围内。
                '[' | '{' | '}' => return None,
                _ => {}
            },
        }
    }
    if !closed || quote.is_some() {
        return None;
    }
    if items.len() == 1 && items[0].trim().is_empty() {
        return Some(Vec::new());
    }
    let last = items.len() - 1;
    let mut packages = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item = item.trim();
        if item.is_empty() {
            // 只允许尾逗号留下的空项。
            if index == last {
                continue;
            }
            return None;
        }
        packages.push(parse_yaml_scalar(item)?);
    }
    Some(packages)
}

fn collect(
    root: &Path,
    directory: &Path,
    patterns: &[String],
    packages: &mut Vec<Package>,
    discovery: &mut Discovery<'_>,
) -> Option<()> {
    if !discovery.ready() || discovery.directories >= 4096 || packages.len() >= 512 {
        return None;
    }
    discovery.directories += 1;
    let mut children = Vec::new();
    for entry in std::fs::read_dir(directory).ok()? {
        if !discovery.ready() || children.len() >= 4096 {
            return None;
        }
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir()
            || matches!(
                entry.file_name().to_str(),
                Some("node_modules" | ".git" | "target" | "dist" | "build")
            )
        {
            continue;
        }
        children.push(entry.path());
    }
    children.sort();
    for child in children {
        let relative = child
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        let positives: Vec<_> = patterns
            .iter()
            .filter(|pattern| !pattern.starts_with('!'))
            .collect();
        if !positives.iter().any(|pattern| {
            let prefix = pattern
                .split(['*', '?', '[', '{'])
                .next()
                .unwrap_or_default()
                .trim_end_matches('/');
            prefix.is_empty()
                || prefix == relative
                || prefix.starts_with(&format!("{relative}/"))
                || relative.starts_with(&format!("{prefix}/"))
        }) {
            continue;
        }
        let matches = positives.iter().any(|pattern| {
            glob::Pattern::new(pattern).is_ok_and(|pattern| {
                pattern.matches_with(
                    &relative,
                    glob::MatchOptions {
                        require_literal_separator: true,
                        require_literal_leading_dot: true,
                        case_sensitive: true,
                    },
                )
            })
        });
        let excluded = patterns
            .iter()
            .filter_map(|pattern| pattern.strip_prefix('!'))
            .any(|pattern| {
                glob::Pattern::new(pattern).is_ok_and(|pattern| {
                    pattern.matches_with(
                        &relative,
                        glob::MatchOptions {
                            require_literal_separator: true,
                            require_literal_leading_dot: true,
                            case_sensitive: true,
                        },
                    )
                })
            });
        if matches && !excluded && child.join("package.json").exists() {
            let json = discovery.manifest(&child.join("package.json"))?;
            packages.push(package(&json, relative));
        }
        collect(root, &child, patterns, packages, discovery)?;
    }
    Some(())
}

fn select(packages: &[Package], context: &Context) -> Option<BTreeSet<usize>> {
    let selectors = &context.project.selectors;
    let only_negative = !selectors.is_empty() && selectors.iter().all(|s| s.starts_with('!'));
    let mut included = if context.project.all || only_negative {
        (0..packages.len()).collect()
    } else {
        BTreeSet::new()
    };
    let mut excluded = BTreeSet::new();
    if context.project.include_root && context.project.manager == PackageManager::Npm {
        included.extend(
            packages
                .iter()
                .enumerate()
                .filter_map(|(index, package)| (package.path == ".").then_some(index)),
        );
    }
    for selector in selectors {
        if context.project.manager == PackageManager::Yarn {
            included.extend(
                packages
                    .iter()
                    .enumerate()
                    .filter_map(|(index, package)| (package.name == *selector).then_some(index)),
            );
            continue;
        }
        let negative = selector.starts_with('!');
        let selector = selector.strip_prefix('!').unwrap_or(selector);
        let dependents = selector.starts_with("...");
        let dependencies = selector.ends_with("...");
        let selector = selector.strip_prefix("...").unwrap_or(selector);
        let selector = selector.strip_suffix("...").unwrap_or(selector);
        let omit_self = selector.starts_with('^') || selector.ends_with('^');
        let selector = selector.trim_matches('^');
        let selector = selector
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(selector);
        // changed-since 选择器需要独立的 Git 查询;绝不把它当成全选。
        if selector.is_empty() || selector.contains(['[', ']']) {
            return None;
        }
        let pattern = glob::Pattern::new(selector.strip_prefix("./").unwrap_or(selector)).ok()?;
        let mut seed: BTreeSet<_> = packages
            .iter()
            .enumerate()
            .filter_map(|(index, package)| {
                (pattern.matches(&package.name)
                    || pattern.matches(&package.path)
                    || context.project.manager == PackageManager::Npm
                        && package.path.starts_with(&format!("{selector}/")))
                .then_some(index)
            })
            .collect();
        if seed.is_empty()
            && context.project.manager == PackageManager::Pnpm
            && !selector.contains('/')
        {
            let unscoped: Vec<_> = packages
                .iter()
                .enumerate()
                .filter_map(|(index, package)| {
                    pattern
                        .matches(package.name.rsplit('/').next().unwrap_or_default())
                        .then_some(index)
                })
                .collect();
            // pnpm 拒绝歧义的无作用域字面量;显式 glob 可以选择多个。
            if unscoped.len() == 1 || selector.contains(['*', '?']) {
                seed.extend(unscoped);
            }
        }
        let mut chosen = seed.clone();
        if dependencies || dependents {
            loop {
                let before = chosen.len();
                for (index, package) in packages.iter().enumerate() {
                    if dependents
                        && package
                            .dependencies
                            .iter()
                            .any(|name| chosen.iter().any(|chosen| packages[*chosen].name == *name))
                    {
                        chosen.insert(index);
                    }
                    if dependencies && chosen.contains(&index) {
                        for (dependency, target) in packages.iter().enumerate() {
                            if package.dependencies.contains(&target.name) {
                                chosen.insert(dependency);
                            }
                        }
                    }
                }
                if chosen.len() == before {
                    break;
                }
            }
        }
        if omit_self {
            chosen = chosen.difference(&seed).copied().collect();
        }
        if negative {
            excluded.extend(chosen);
        } else {
            included.extend(chosen);
        }
    }
    Some(included.difference(&excluded).copied().collect())
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use slterm_completions::command_context::ShellSyntax;

    /// 窄解析器覆盖 pnpm 声明子集,并与旧 serde_yaml 行为保持等价。
    #[test]
    fn pnpm_packages_parser_accepts_declared_subset_and_rejects_malformed_input() {
        let cases: &[(&str, Option<&[&str]>)] = &[
            // 块序列 + 单/双引号 + 行尾注释(pnpm 文档典型写法)。
            (
                "packages:\n  # 直接子目录\n  - 'packages/*'\n  - \"components/**\" # 行尾注释\n  - '!**/test/**'\n",
                Some(&["packages/*", "components/**", "!**/test/**"]),
            ),
            // 与键同缩进的块序列。
            ("packages:\n- a\n- b\n", Some(&["a", "b"])),
            // 行内数组:裸值、单引号、双引号混排。
            (
                "packages: [packages/*, 'components/**', \"!**/test/**\"]\n",
                Some(&["packages/*", "components/**", "!**/test/**"]),
            ),
            // 尾逗号合法,空数组合法。
            ("packages: [a, b,]\n", Some(&["a", "b"])),
            ("packages: []\n", Some(&[])),
            ("packages: [ ]\n", Some(&[])),
            // 其它顶层键及其嵌套内容不参与解析。
            (
                "onlyBuiltDependencies:\n  - esbuild\npackages:\n  - p/*\n",
                Some(&["p/*"]),
            ),
            // 无 packages 键 → pnpm 默认全量匹配。
            ("name: root\ncatalog:\n  react: ^19\n", Some(&["**"])),
            // 文档分隔符忽略。
            ("---\npackages:\n  - a\n", Some(&["a"])),
            // 空值/null 不是序列,与 as_sequence()? 语义一致。
            ("packages:\n", None),
            ("packages: # nothing\n", None),
            // 标量/映射值不是序列。
            ("packages: 42\n", None),
            ("packages:\n  key: value\n", None),
            // 畸形输入:未闭合引号与未闭合数组。
            ("packages:\n  - 'unclosed\n", None),
            ("packages: [a, 'unclosed]\n", None),
            ("packages: [a b\n", None),
            ("packages: [a, , b]\n", None),
            // 重复键按 serde_yaml 语义拒绝。
            ("packages:\n  - a\npackages:\n  - b\n", None),
            // 引号内的 # 不算注释;整行注释与空行忽略。
            (
                "\n# 全行注释\npackages:\n  - 'a#b' # 引号内 # 不算注释\n\n  - c\n",
                Some(&["a#b", "c"]),
            ),
        ];
        for (text, expected) in cases {
            let actual = parse_pnpm_packages(text);
            let expected = expected.map(|items| {
                items
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect::<Vec<_>>()
            });
            assert_eq!(actual, expected, "input: {text:?}");
        }
    }

    #[test]
    fn pnpm_uses_its_workspace_root_and_unscoped_names_without_borrowing_npm_patterns() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"name":"fixture-root","workspaces":["wrong/*"],"scripts":{"root-only":"touch forbidden"}}"#,
        )
        .unwrap();
        let app = root.path().join("packages/app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(
            app.join("package.json"),
            r#"{"name":"@fixture/app","scripts":{"app-only":"touch forbidden"}}"#,
        )
        .unwrap();
        let names = |cwd: &Path, line: &str| {
            let context = Context::parse(line, line.len(), ShellSyntax::Posix).unwrap();
            read(cwd, &context, &|| false).unwrap_or_default()
        };
        assert_eq!(
            names(root.path(), "pnpm --filter app run app"),
            ["app-only"]
        );
        assert_eq!(
            names(root.path(), "pnpm --filter fixture-root run ro"),
            ["root-only"]
        );
        std::fs::write(
            root.path().join("pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n",
        )
        .unwrap();
        assert_eq!(names(&app, "pnpm --filter app run app"), ["app-only"]);
        assert!(names(root.path(), "pnpm --filter fixture-root run ro").is_empty());
        let other = root.path().join("packages/other");
        std::fs::create_dir(&other).unwrap();
        std::fs::write(
            other.join("package.json"),
            r#"{"name":"@other/app","scripts":{"other-only":"touch forbidden"}}"#,
        )
        .unwrap();
        assert!(names(root.path(), "pnpm --filter app run app").is_empty());
        assert!(!root.path().join("forbidden").exists());
    }

    #[test]
    fn workspace_selection_uses_declared_packages_and_never_executes_scripts() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"name":"root","workspaces":["packages/*","!packages/excluded"],"scripts":{"root-only":"touch forbidden"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n  - '!packages/excluded'\n",
        )
        .unwrap();
        for (name, scripts, deps) in [
            (
                "app",
                r#"{"build:app":"touch forbidden","shared":"touch forbidden"}"#,
                r#"{"lib":"workspace:*"}"#,
            ),
            (
                "lib",
                r#"{"build:lib":"touch forbidden","shared":"touch forbidden"}"#,
                "{}",
            ),
            ("excluded", r#"{"build:excluded":"touch forbidden"}"#, "{}"),
        ] {
            let path = root.path().join("packages").join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                path.join("package.json"),
                format!(r#"{{"name":"{name}","scripts":{scripts},"dependencies":{deps}}}"#),
            )
            .unwrap();
        }
        let names = |line: &str| {
            let context = Context::parse(line, line.len(), ShellSyntax::Posix).unwrap();
            read(root.path(), &context, &|| false).unwrap_or_default()
        };
        assert_eq!(names("npm --workspace a"), ["app", "lib"]);
        assert_eq!(names("npm -w app run bu"), ["build:app", "shared"]);
        assert_eq!(names("npm run --workspace app bu"), ["build:app", "shared"]);
        assert_eq!(names("npm --workspaces run sh"), ["shared"]);
        assert_eq!(
            names("npm --workspaces --if-present run bu"),
            ["build:app", "build:lib", "shared"]
        );
        assert_eq!(
            names("pnpm --filter app... run sh"),
            ["build:app", "build:lib", "shared"]
        );
        assert!(names("npm -w app --include-workspace-root run bu").is_empty());
        assert_eq!(
            names("npm -w app --include-workspace-root --if-present run bu"),
            ["build:app", "root-only", "shared"]
        );
        assert_eq!(
            names("pnpm --filter 'app^...' run bu"),
            ["build:lib", "shared"]
        );
        assert_eq!(names("yarn workspace app run bu"), ["build:app", "shared"]);
        assert_eq!(names("yarn workspace app bu"), ["build:app", "shared"]);
        assert!(names("npm --workspace missing run bu").is_empty());
        assert!(!root.path().join("forbidden").exists());
        let context = Context::parse("npm --workspace a", 17, ShellSyntax::Posix).unwrap();
        assert!(read(root.path(), &context, &|| true).is_none());
    }
}
