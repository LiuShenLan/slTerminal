# 1. CI 验证（GitHub Actions）

```
PS D:\data\learn\code\slTerminal> gh run view
? Select a workflow run X Phase 0 工程与测试基建, CI [main] 20m15s ago

X main CI · 27763999006
Triggered via push about 20 minutes ago

JOBS
X build-and-test in 5m24s (ID 82145606590)
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


To see what failed, try: gh run view 27763999006 --log-failed
View this run on GitHub: https://github.com/LiuShenLan/slTerminal/actions/runs/27763999006
```


# 2. L4 E2E 测试（条件性）

1. 确认 WebView2 与 msedgedriver 版本对齐

```powershell
PS D:\data\learn\code\slTerminal> Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" | Select-Object pv


pv
--
148.0.3967.54
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

✓ built in 108ms
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
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.14s
       Built application at: D:\data\learn\code\slTerminal\src-tauri\target\debug\slterminal.exe
PS D:\data\learn\code\slTerminal> Test-Path "src-tauri\target\debug\slterminal.exe"
True
```

3. 运行 E2E 测试

```powershell
PS D:\data\learn\code\slTerminal> npm ci

added 187 packages in 3s

55 packages are looking for funding
  run `npm fund` for details
PS D:\data\learn\code\slTerminal> npx wdio run wdio.conf.js

                 -:...........................-:.
                 +                              +
              `` +      `...`        `...`      + `
            ./+/ +    .:://:::`    `::///::`  ` + ++/.
           .+oo+ +    /:+ooo+-/    /-+ooo+-/ ./ + +oo+.
           -ooo+ +    /-+ooo+-/    /-+ooo+-/ .: + +ooo.
            -+o+ +    `::///:-`    `::///::`    + +o+-
             ``. /.     `````        `````     .: .``
                  .----------------------------.
           `-::::::::::::::::::::::::::::::::::::::::-`
          .+oooo/:------------------------------:/oooo+.
      `.--/oooo-                                  :oooo/--.`
    .::-``:oooo`                                  .oooo-``-::.
  ./-`    -oooo`--.: :.--                         .oooo-    `-/.
 -/`    `-/oooo////////////////////////////////////oooo/.`    `/-
`+`   `/+oooooooooooooooooooooooooooooooooooooooooooooooo+:`   .+`
-/    +o/.:oooooooooooooooooooooooooooooooooooooooooooo:-/o/    +.
-/   .o+  -oooosoooososssssooooo------------------:oooo- `oo`   +.
-/   .o+  -oooodooohyyssosshoooo`                 .oooo-  oo.   +.
-/   .o+  -oooodooysdooooooyyooo` `.--.``     .:::-oooo-  oo.   +.
-/   .o+  -oooodoyyodsoooooyyooo.//-..-:/:.`.//.`./oooo-  oo.   +.
-/   .o+  -oooohsyoooyysssysoooo+-`     `-:::.    .oooo-  oo.   +.
-/   .o+  -ooooosooooooosooooooo+//////////////////oooo-  oo.   +.
-/   .o+  -oooooooooooooooooooooooooooooooooooooooooooo-  oo.   +.
-/   .o+  -oooooooooooooooooooooooooooooooooooooooooooo-  oo.   +.
-+////o+` -oooo---:///:----://::------------------:oooo- `oo////+-
+ooooooo/`-oooo``:-```.:`.:.`.+/-    .::::::::::` .oooo-`+ooooooo+
oooooooo+`-oooo`-- `/` .:+  -/-`/`   .::::::::::  .oooo-.+oooooooo
+-/+://-/ -oooo-`:`.o-`:.:-````.:    .///:``````  -oooo-`/-//:+:-+
: :..--:-:.+ooo+/://o+/-.-:////:-....-::::-....--/+ooo+.:.:--.-- /
- /./`-:-` .:///+/ooooo/+///////////////+++ooooo/+///:. .-:.`+./ :
:-:/.           :`ooooo`/`              .:.ooooo :           ./---
                :`ooooo`/`              .:.ooooo :
                :`ooooo./`              .:-ooooo :
                :`ooooo./`              .:-ooooo :
            `...:-+++++:/.              ./:+++++-:...`
           :-.````````/../              /.-:````````.:-
          -/::::::::://:/+             `+/:+::::::::::+.
          :oooooooooooo++/              +++oooooooooooo-

                           Webdriver.IO
              Next-gen browser and mobile automation
                    test framework for Node.js


Creating WebdriverIO project in D:\data\learn\code\slTerminal\run

Installing packages:  @wdio/cli


added 391 packages in 6s

145 packages are looking for funding
  run `npm fund` for details

Finished installing packages.

Running WDIO CLI Wizard...

===============================
🤖 WDIO Configuration Wizard 🧙
===============================

? A project named "slterminal" was detected at "D:\data\learn\code\slTerminal", cor✔ A project named "slterminal" was detected at "D:\data\learn\code\slTerminal",
correct? Yes
✔ What type of testing would you like to do? Desktop Testing - of Electron, Tauri, or macOS Applications
    > https://webdriver.io/docs/desktop-testing
✔ What type of desktop application are you testing? Tauri (https://tauri.app/)
✔ Which WebDriver provider would you like to use for Tauri? Official tauri-driver (cross-platform, Cargo-installed)
✔ Install @wdio/tauri-plugin for richer in-webview integration (browser.tauri.execute, mocking)? Yes
✔ Path to your built Tauri binary (leave blank to use the default `cargo tauri build` output)?
✔ Which framework do you want to use? Mocha (https://mochajs.org/)
✔ Do you want to use Typescript to write tests? Yes
✔ Do you want WebdriverIO to autogenerate some test files? Yes
✔ What should be the location of your spec files? D:\data\learn\code\slTerminal\test\specs\**\*.ts
✔ Which reporter do you want to use? spec
✔ Do you want to add a plugin to your test setup?
✔ Do you want to add a service to your test setup? tauri, tauri-plugin
✔ Do you want me to run `npm install` Yes


Installing packages using npm:
- @wdio/local-runner@latest
- @wdio/mocha-framework@latest
- @wdio/spec-reporter@latest
- @wdio/tauri-service@latest
- @wdio/tauri-plugin@latest
- @types/node
- @wdio/globals@latest
- expect-webdriverio


npm warn deprecated inflight@1.0.6: This module is not supported, and leaks memory. Do not use it. Check out lru-cache if you want a good and tested way to coalesce async requests by a key value, which is much more comprehensive and powerful.
npm warn deprecated glob@8.1.0: Glob versions prior to v9 are no longer supported

added 97 packages in 3s

163 packages are looking for funding
  run `npm fund` for details
✔ Success!

Creating a WebdriverIO config file...
✔ Success!

Autogenerate test files...
✔ Success!

Adding "wdio" script to package.json
✔ Success!

🤖 Successfully setup project at D:\data\learn\code\slTerminal 🎉

Join our Discord Community Server and instantly find answers to your issues or queries. Or just join and say hi 👋!
  🔗 https://discord.webdriver.io

Visit the project on GitHub to report bugs 🐛 or raise feature requests 💡:
  🔗 https://github.com/webdriverio/webdriverio

🦀 Tauri requires a couple of Rust-side additions before tests can run:

  1. Install the official tauri-driver via Cargo:

       cargo install tauri-driver --locked

  2. On Linux, set `WEBKIT_DISABLE_DMABUF_RENDERER=1` in your shell when running tests.

  Frontend plugin (@wdio/tauri-plugin) was added to your npm dependencies.
  See https://webdriver.io/docs/desktop-testing/tauri/plugin-setup for wiring instructions.

Full Tauri setup docs:
  🔗 https://webdriver.io/docs/desktop-testing/tauri
To run your tests, execute:
$ cd D:\data\learn\code\slTerminal
$ npm run wdio

Adding scripts to package.json
node:internal/modules/cjs/loader:1502
  const err = new Error(message);
              ^

Error: Cannot find module 'D:\data\learn\code\slTerminal\run\package.json'
Require stack:
- D:\cache\nodejs\node_cache\_npx\471611576716610c\node_modules\wdio\build\index.js
- D:\cache\nodejs\node_cache\_npx\471611576716610c\node_modules\wdio\bin\wdio.js
    at Module._resolveFilename (node:internal/modules/cjs/loader:1502:15)
    at wrapResolveFilename (node:internal/modules/cjs/loader:1073:27)
    at defaultResolveImplForCJSLoading (node:internal/modules/cjs/loader:1097:10)
    at resolveForCJSWithHooks (node:internal/modules/cjs/loader:1118:12)
    at Module._load (node:internal/modules/cjs/loader:1287:25)
    at wrapModuleLoad (node:internal/modules/cjs/loader:255:19)
    at Module.require (node:internal/modules/cjs/loader:1602:12)
    at require (node:internal/modules/helpers:191:16)
    at createWebdriverIO (D:\cache\nodejs\node_cache\_npx\471611576716610c\node_modules\wdio\build\index.js:81:21) {
  code: 'MODULE_NOT_FOUND',
  requireStack: [
    'D:\\cache\\nodejs\\node_cache\\_npx\\471611576716610c\\node_modules\\wdio\\build\\index.js',
    'D:\\cache\\nodejs\\node_cache\\_npx\\471611576716610c\\node_modules\\wdio\\bin\\wdio.js'
  ]
}

Node.js v26.1.0
```

# 3. 人工验证

1. 前提条件

```powershell
PS D:\data\learn\code\slTerminal\src-tauri> cargo check
   Compiling cfg-if v1.0.4
   ...
    Checking tauri-runtime-wry v2.11.3
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
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.77s
```

2. 启动应用

```powershell
PS D:\data\learn\code\slTerminal> npm run tauri dev

> slterminal@0.1.0 tauri
> tauri dev

     Running BeforeDevCommand (`npm run dev`)

> slterminal@0.1.0 dev
> vite


  VITE v8.0.16  ready in 549 ms

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
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.14s
     Running `target\debug\slterminal.exe`
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
libpng warning: iCCP: known incorrect sRGB profile
```

3. 肉眼确定清单

|#|检查项|通过标准|确认结果|
|:-:|:-:|:-:|:-:|
|A|窗口以暗色主题呈现|窗口背景色为暗色（非白色/亮色），标题栏、滚动条等 chrome 元素也跟随暗色|通过|
|B|Dockview 区域填满窗口|窗口客户区无空白/白边，Dockview 布局容器填充整个可用区域；布局为空（无面板标签页），显示暗色空状态|通过|
|C|控制台无报错|启动 Tauri dev 的终端输出中无 ERROR、panic、unreachable 等异常；WebView2 DevTools 控制台无红色错误（按 F12 或 Ctrl+Shift+I 打开）|控制台报错:`libpng warning: iCCP: known incorrect sRGB profile`|