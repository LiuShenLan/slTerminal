//! 翻译模板替换:参数值字面插入,绝不二次解析为占位符(防注入式串扰)。
//!
//! 语义:`{name}` 用同名参数替换;缺失参数原样保留;`{{`/`}}` 转义为
//! 单个花括号;未成对的 `{`(无闭合)原样保留;孤立 `}` 原样输出。
//! 输出容量按模板长度 + 参数值总长预分配。

pub(super) fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let extra = args.iter().map(|(_, value)| value.len()).sum::<usize>();
    let mut output = String::with_capacity(template.len().saturating_add(extra));
    let mut rest = template;
    while let Some(position) = rest.find(['{', '}']) {
        output.push_str(&rest[..position]);
        rest = &rest[position..];
        if rest.starts_with("{{") || rest.starts_with("}}") {
            output.push_str(&rest[..1]);
            rest = &rest[2..];
            continue;
        }
        if !rest.starts_with('{') {
            output.push('}');
            rest = &rest[1..];
            continue;
        }
        let Some(end) = rest.find('}') else { break };
        let name = &rest[1..end];
        let replacement = args.iter().find(|(argument, _)| *argument == name);
        output.push_str(
            replacement
                .map(|(_, value)| *value)
                .unwrap_or(&rest[..end + 1]),
        );
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod format_tests {
    use super::substitute;

    /// 参数值里的占位符形态是字面文本,不得再次展开。
    #[test]
    fn argument_values_are_never_interpreted_as_more_placeholders() {
        assert_eq!(
            substitute(
                "{name}: {status}",
                &[("name", "{status}"), ("status", "200")]
            ),
            "{status}: 200"
        );
    }

    /// Unicode、缺失参数、转义花括号与未闭合模板原样保留。
    #[test]
    fn preserves_unicode_missing_arguments_and_escaped_braces() {
        assert_eq!(
            substitute("你好 {name} / {missing} {{code}}", &[("name", "世界")]),
            "你好 世界 / {missing} {code}"
        );
        assert_eq!(substitute("unfinished {name", &[]), "unfinished {name");
        assert_eq!(substitute("stray } brace", &[]), "stray } brace");
    }
}
