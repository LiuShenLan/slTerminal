# 依赖与构建基线

> 固定可复现的依赖、语法资产、GPUI fork 与发布审计输入；版本核验锚为 2026-10-24。

## 决策与红线

### 工具链与固定 fork

- 目标工具链以根 Cargo 配置和实测环境为准：`rustc 1.97.1`、`cargo 1.97.1`，Windows x64 MSVC。依赖升级是独立改动，不与功能重写混合。
- GPUI 只使用审计后的固定 fork，不升级版本线。审计输入为 `LiuShenLan/zed` rev `398ddb60f3d188af95e3661b1b1de8be0cbebbd7`、`LiuShenLan/gpui-kit` rev `d71650dd3f99d89cffa243147ed4c2f103974401`。
- GPUI 补丁做减法：保留仍必要的 Windows、输入、IME、GPU 与生命周期修复，删除旧编辑器、Markdown、数学覆盖层和不再需要的桥接补丁。新 rev 必须与补丁审计表同提交固定。
- 不为维持旧安装包体积删除规格语言；先测全量基线，再依发布规则说明并重钉体积预算。

### 新增直接依赖

以下版本是目标实现的精确起点；`Cargo.lock`、构建脚本、transitive license 和 notice 必须一并审计。未列出的既有依赖沿用经审计的 workspace 锁，不因本表而隐式升级。

| 依赖 | 版本 | 用途与边界 | 许可核验 |
|---|---|---|---|
| `ropey` | `2.0.0-beta.1` | Document Rope；不进入 UI 自持文本。 | MIT／Apache-2.0 |
| `comrak` | `0.57.0` | 唯一 Markdown 语义解析器。 | BSD-2-Clause |
| `similar` | `3.2.1` | Myers 行差分；deadline 近似结果必须丢弃。 | Apache-2.0 |
| `rusqlite` | `0.41.0` | SQLite，启用 `bundled`、`backup`。 | MIT；bundled SQLite notice 随包 |
| `encoding_rs` | `0.8.42` | 有界编码解码与保存转换。 | MIT／Apache-2.0／BSD-3-Clause |
| `chardetng` | `1.0.0` | 未知编码的有限候选检测。 | MIT／Apache-2.0 |
| `yaml-rust2` | `0.13.0` | front matter 事件式有界校验。 | MIT／Apache-2.0 |
| `schemars` | `1.2.3` | Runtime 可选 `schema` feature。 | MIT |
| `uuid` | `1.28.0` | UUIDv7 稳定身份。 | MIT／Apache-2.0 |
| `html5ever` | `0.27.0` | 真实 HTML TreeSink，不用正则清洗。 | MIT／Apache-2.0 |
| `quick-xml` | `0.41.0` | 禁用 DTD／外部实体的 SVG XML 清洗。 | MIT |
| `usvg`、`resvg` | `0.46.0` | 清洗后 SVG 静态光栅化；不接收源 XML。 | MIT／Apache-2.0 |
| `tiny-skia` | `0.11.4` | 光栅化底层与位图生成。 | BSD-3-Clause |
| `image` | `0.25.10` | 图片解码与有界帧读取。 | MIT／Apache-2.0 |
| `tree-sitter` | `0.27.1` | 增量语法、InputEdit、可中止解析。 | MIT |
| `proptest` | `1.12.0` | 仅开发依赖：布局、编辑、协议和恢复属性测试。 | MIT／Apache-2.0 |
| `jsonschema` | `0.60.1` | 仅开发依赖：生成 schema 与 conformance 校验。 | MIT |

`image` 关闭默认格式与并行特性，只启用规格需要的 PNG、JPEG、GIF、WebP、BMP、ICO 和 `avif-native` 解码；不启用 ravif 编码。AVIF 的 dav1d、mp4parse 及其上游 notice 必须通过 Windows x64 构建、MPL notice 和静态链接门禁；Rust 包装 crate 的许可元数据不能替代上游审计。上游审计锚为 dav1d `1.5.0`、commit `32cf02af50f32af108a3b281c452788dccdac648`（2026-10-26 对远端 tag 解引用核验）。`dav1d-sys 0.8.3` 使用仓内精确源码补丁，仅在 Meson setup 增加 `--vsenv`，防止 PATH 中的 MinGW 与 MSVC Rust 目标混链。`.cargo/config.toml` 强制 `dav1d-sys` 内部构建并保留 static 链接指令，产品构建后以 `dumpbin /dependents` 拒绝 dav1d DLL；构建机须提供 Git、Meson、Ninja、NASM 与 pkg-config；CI 固定后四项版本，源码只在 Cargo `OUT_DIR` 的独立目录构建。

`zip` 仅保留给隔离记录与诊断 ZIP 导出，不再是主题包格式。HTTP、密码学、正则和公式既有依赖继续服务对应目标能力；删除旧 AI／Markdown 依赖前必须核对隔离导出与公式是否仍使用其传递功能。

### LanguageRegistry 与 grammar

每种语言必须同时登记扩展名、固定文件名、Markdown 别名、grammar、highlight query 和折叠规则。规格语言缺一资产即阻塞并报告；只有未注册类型使用纯文本，不把缺 grammar 静默伪装成完成。

| 语言或别名组 | crate 与精确版本 | 许可 |
|---|---|---|
| JavaScript | `tree-sitter-javascript 0.25.0` | MIT |
| TypeScript／TSX | `tree-sitter-typescript 0.23.2` | MIT |
| Python | `tree-sitter-python 0.25.0` | MIT |
| Rust | `tree-sitter-rust 0.24.2` | MIT |
| JSON | `tree-sitter-json 0.24.8` | MIT |
| JSONC | `tree-sitter-json5 0.1.0` | MIT |
| HTML | `tree-sitter-html 0.23.2` | MIT |
| CSS | `tree-sitter-css 0.25.0` | MIT |
| SCSS | `tree-sitter-scss 1.0.0` | MIT |
| Less | `tree-sitter-less 1.0.0` | MIT |
| Markdown 源码 | `tree-sitter-md 0.5.4` | MIT |
| XML | `tree-sitter-xml 0.7.0` | MIT |
| Shell | `tree-sitter-bash 0.25.1` | MIT |
| PowerShell | `tree-sitter-powershell 0.26.4` | MIT |
| Go | `tree-sitter-go 0.25.0` | MIT |
| Java | `tree-sitter-java 0.23.5` | MIT |
| C | `tree-sitter-c 0.24.2` | MIT |
| C++ | `tree-sitter-cpp 0.23.4` | MIT |
| C# | `tree-sitter-c-sharp 0.23.5` | MIT |
| SQL | `tree-sitter-sequel 0.3.11` | MIT |
| YAML | `tree-sitter-yaml 0.7.2` | MIT |
| TOML | `tree-sitter-toml-ng 0.7.0` | MIT |
| INI | `tree-sitter-ini 1.4.0` | Apache-2.0 |
| Diff | `tree-sitter-diff 0.1.0` | MIT |
| Dockerfile | `tree-sitter-dockerfile 0.2.0` | MIT |
| Make | `tree-sitter-make 1.1.1` | MIT |

JSONC 使用 JSON5 仅作高亮语法超集，不用于验证 JSONC；Markdown 语义仍只走 Comrak，Tree-sitter Markdown 不参与渲染语义或第二套 injection 规则。所有 grammar 必须验证 ABI 13–15、Windows x64 编译和 highlights query 加载。

### 依赖删减与许可

- 删除 AI Provider／Assistant、模型管理、Merge、终端数学覆盖层、旧 Markdown 双解析、旧主题 ZIP 包和旧 Runtime 的专属依赖；保留公式、隔离导出和诊断实际需要的依赖。
- `pulldown-cmark`、旧 Markdown 渲染桥和不再被公式使用的转义依赖不得因历史引用留在生产图；删除前以 Cargo tree 和实际符号引用核对。
- 许可审计以锁定版本的源码包、`LICENSE*`、`NOTICE*` 和构建脚本为准。候选许可初查为 MIT／Apache／BSD／MPL 系列，不以此替代完整分发审计。
- 第三方 C/C++ 构建只在独立依赖构建目录执行，不在应用工作目录加载未审计脚本；构建产物和 notice 进入发布清单。
- 界面与品牌资产沿用 featureSpec 资产许可边界：lucide-static 的 ISC／Feather MIT 文本、CLI 品牌授权和项目自绘许可均进入发布审计；预览色不进入运行时主题 token。

## 验收与测试

- `cargo tree`、锁文件和依赖方向门禁确认无 AI Provider、旧 Runtime、旧主题包和第二 Markdown 语义依赖。
- 每个 grammar 在干净临时目录完成 Windows x64 构建、ABI 检查、highlight query 编译和最小语言样例解析。
- `schema` feature 独立构建协议生成器；默认协议构建不引入 schemars 或 schema 生成二进制。
- 图片构建矩阵覆盖 PNG、JPEG、GIF、WebP、AVIF 解码与禁用编码；损坏、超大和恶意压缩样本受解码预算约束。
- 依赖许可测试检查每个生产 crate 和静态链接上游均有可定位许可文本；缺失即阻塞发布。
- 记录新全量安装包体积、二进制依赖和压缩结果，形成带日期与 commit 的发布基线，不用旧预算掩盖变化。

## 操作

- 依赖变更先更新本表、锁文件、许可清单和架构门禁，再运行构建与全量测试。
- grammar 缺失时报告具体语言和资产，不临时以纯文本替代；确需删减规格必须重新取得用户裁决。
- GPUI 补丁变更同时提交审计说明、上游 rev 和 Windows 实测证据。
