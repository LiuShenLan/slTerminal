# 1. CI 验证（GitHub Actions）

```powershell
PS D:\data\learn\code\slTerminal> gh run view
? Select a workflow run X Phase 0 工程与测试基建, CI [main] 6m35s ago

X main CI · 27766740300
Triggered via push about 6 minutes ago

JOBS
X build-and-test in 5m5s (ID 82155422941)
  ✓ Set up job
  ✓ Run actions/checkout@v4
  ✓ Setup Node.js
  ✓ Setup Rust toolchain
  ✓ Cache Rust dependencies
  ✓ Install frontend dependencies
  ✓ Install msedgedriver
  ✓ Lint check
  ✓ Frontend build
  ✓ Frontend tests (Vitest L2 + L3)
  X Backend build
  - Backend tests (L1)
  - Tauri debug build
  - E2E tests (L4)
  - Post Cache Rust dependencies
  ✓ Post Setup Rust toolchain
  - Post Setup Node.js
  ✓ Post Run actions/checkout@v4
  ✓ Complete job

ANNOTATIONS
! Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4, actions/setup-node@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/
build-and-test: .github#3

X Process completed with exit code 1.
build-and-test: .github#557

X variants `Io`, `Pty`, `Git`, `Serde`, and `Unknown` are never constructed
build-and-test: .github#8


To see what failed, try: gh run view 27766740300 --log-failed
View this run on GitHub: https://github.com/LiuShenLan/slTerminal/actions/runs/27766740300
```


# 2. L4 E2E 测试（条件性）

1. 确认 WebView2 与 msedgedriver 版本对齐

```powershell
PS D:\data\learn\code\slTerminal> Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" | Select-Object pv


pv
--
148.0.3967.54

PS D:\data\learn\code\slTerminal> msedgedriver --version
msedgedriver: The term 'msedgedriver' is not recognized as a name of a cmdlet, function, script file, or executable program.
Check the spelling of the name, or if a path was included, verify that the path is correct and try again.

[General Feedback]
  The command "msedgedriver" was not found, but does exist in the current location.
  PowerShell does not load commands from the current location by default (see 'Get-Help about_Command_Precedence').

  If you trust this command, run the following command instead:
    ➤ .\msedgedriver

PS D:\data\learn\code\slTerminal> .\msedgedriver
Starting msedgedriver 148.0.3967.54 (c994e204927e169f407ef8cd1bd301774deefc6e) on port 0
To submit feedback, report a bug, or suggest new features, please visit https://github.com/MicrosoftEdge/EdgeWebDriver

Only local connections are allowed.
Please see https://aka.ms/WebDriverSecurity for suggestions on keeping Microsoft Edge WebDriver safe.
msedgedriver was started successfully on port 7780.
```

2. 构建应用（debug 模式）

```powershell
PS D:\data\learn\code\slTerminal> npm run tauri build -- --debug --no-bundle

> slterminal@0.1.0 tauri
> tauri build --debug --no-bundle

        Info Looking up installed tauri packages to check mismatched versions...
     Running beforeBuildCommand `npm run build`

> slterminal@0.1.0 build
> tsc && vite build

vite v8.0.16 building client environment for production...
✓ 20 modules transformed.
computing gzip size...
dist/index.html                   0.54 kB │ gzip:   0.35 kB
dist/assets/index-D8LP7lKz.css   95.47 kB │ gzip:   8.47 kB
dist/assets/index-RmoU1lxp.js   495.86 kB │ gzip: 126.58 kB

✓ built in 164ms
   Compiling slterminal v0.1.0 (D:\data\learn\code\slTerminal\src-tauri)
warning: variants `Io`, `Pty`, `Git`, `Serde`, and `Unknown` are never constructed
  --> src\error.rs:8:5
   |
 6 | pub enum AppError {
   |          -------- variants in this enum
 7 |     #[error("IO 错误: {0}")]
 8 |     Io(String),
   |     ^^
...
11 |     Pty(String),
   |     ^^^
...
14 |     Git(String),
   |     ^^^
...
17 |     Serde(String),
   |     ^^^^^
...
20 |     Unknown(String),
   |     ^^^^^^^
   |
   = note: `AppError` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `slterminal` (lib) generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.28s
       Built application at: D:\data\learn\code\slTerminal\src-tauri\target\debug\slterminal.exe
PS D:\data\learn\code\slTerminal> Test-Path "src-tauri\target\debug\slterminal.exe"

True
```

3. 运行 E2E 测试

```powershell
PS D:\data\learn\code\slTerminal> npm run wdio

> slterminal@0.1.0 wdio
> wdio run ./wdio.conf.ts

2026-06-18T14:34:38.103Z INFO @wdio/local-runner: Shutting down spawned worker
2026-06-18T14:34:38.361Z INFO @wdio/local-runner: Waiting for 0 to shut down gracefully
2026-06-18T14:34:38.361Z INFO @wdio/local-runner: shutting down
2026-06-18T14:34:38.362Z INFO @wdio/cli:launcher: Run onComplete hook
Error: Failed to initialise launcher service unknown: Error: Couldn't find plugin "tauri-plugin" service, neither as wdio scoped package "@wdio/tauri-plugin-service" nor as community package "wdio-tauri-plugin-service". Please make sure you have it installed!
    at initializePlugin (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/utils/build/index.js:556:9)
    at async initializeServices (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/utils/build/index.js:606:21)
    at async initializeLauncherService (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/utils/build/index.js:619:22)
    at async Launcher.run (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/cli/build/index.js:856:59)
    at initializeLauncherService (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/utils/build/index.js:640:11)
    at async Launcher.run (file:///D:/data/learn/code/slTerminal/node_modules/@wdio/cli/build/index.js:856:59)
```

# 3. 人工验证

1. 启动应用

```powershell
PS D:\data\learn\code\slTerminal> npm run tauri dev

> slterminal@0.1.0 tauri
> tauri dev

     Running BeforeDevCommand (`npm run dev`)

> slterminal@0.1.0 dev
> vite


  VITE v8.0.16  ready in 127 ms

  ➜  Local:   http://localhost:1420/
     Running DevCommand (`cargo  run --no-default-features --color always --`)
        Info Watching D:\data\learn\code\slTerminal\src-tauri for changes...
   Compiling slterminal v0.1.0 (D:\data\learn\code\slTerminal\src-tauri)
warning: variants `Io`, `Pty`, `Git`, `Serde`, and `Unknown` are never constructed
  --> src\error.rs:8:5
   |
 6 | pub enum AppError {
   |          -------- variants in this enum
 7 |     #[error("IO 错误: {0}")]
 8 |     Io(String),
   |     ^^
...
11 |     Pty(String),
   |     ^^^
...
14 |     Git(String),
   |     ^^^
...
17 |     Serde(String),
   |     ^^^^^
...
20 |     Unknown(String),
   |     ^^^^^^^
   |
   = note: `AppError` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `slterminal` (lib) generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.13s
     Running `target\debug\slterminal.exe`
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
```

2. 肉眼确定清单

|#|检查项|通过标准|确认结果|
|:-:|:-:|:-:|:-:|
|A|窗口以暗色主题呈现|窗口背景色为暗色（非白色/亮色），标题栏、滚动条等 chrome 元素也跟随暗色|通过|
|B|Dockview 区域填满窗口|窗口客户区无空白/白边，Dockview 布局容器填充整个可用区域；布局为空（无面板标签页），显示暗色空状态|通过|
|C|控制台无报错|启动 Tauri dev 的终端输出中无 ERROR、panic、unreachable 等异常；WebView2 DevTools 控制台无红色错误（按 F12 或 Ctrl+Shift+I 打开）|控制台报错:`libpng warning: iCCP: known incorrect sRGB profile`|