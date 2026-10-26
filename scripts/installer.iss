; slTerminal Inno Setup 安装器定义(全参数化:脚本经 /D 注入,零硬编码版本)。
; 构建入口:scripts/build-installer.ps1;发布入口:scripts/release.ps1。

#ifndef AppVersion
  #define AppVersion "0.3.0"
#endif
#ifndef NumericVersion
  #define NumericVersion "0.3.0.0"
#endif
#ifndef Configuration
  #define Configuration "release"
#endif
#ifndef Architecture
  #define Architecture "x64"
#endif
#ifndef RepoRoot
  #define RepoRoot ".."
#endif
#ifndef BuildRoot
  #define BuildRoot RepoRoot + "\target\" + Configuration
#endif
#ifndef OutputDir
  #define OutputDir RepoRoot + "\dist"
#endif
#ifndef IslFile
  #define IslFile RepoRoot + "\target\installer-tools\ChineseSimplified.isl"
#endif

; 夹具模式:隔离 AppId/AppName/输出名/注册表键,自测不污染真实卸载项。
; GUID 一经钉死永不变更。
#ifdef AcceptanceFixture
  #define AppIdValue "{45EED1CD-AE5E-4D5C-84F2-8D1949722E9E}"
  #define AppDisplayName "slTerminal Update Acceptance"
  #define AppUserModelId "com.slterminal.terminal.acceptance"
#else
  #define AppIdValue "{D9D92995-C831-4610-87BC-4B82B43B3E71}"
  #define AppDisplayName "slTerminal"
  #define AppUserModelId "com.slterminal.terminal"
#endif

[Setup]
AppId={{#AppIdValue}
AppName={#AppDisplayName}
AppVersion={#AppVersion}
AppVerName={#AppDisplayName} {#AppVersion}
AppPublisher=LiuShenLan
AppUpdatesURL=https://github.com/LiuShenLan/slTerminal/releases
VersionInfoVersion={#NumericVersion}
#ifdef AcceptanceFixture
DefaultDirName={autopf}\slTerminal Acceptance
#else
DefaultDirName={autopf}\slTerminal
#endif
UsePreviousAppDir=yes
UsePreviousGroup=no
DisableProgramGroupPage=yes
; 显式组名：缺省会落在 Programs\(Default)。
DefaultGroupName={#AppDisplayName}
; 用户级安装:写 HKCU、无需管理员,咬合单窗口单实例定位。
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
; Win10 1809 下界 = 产品定位下界。
MinVersion=10.0.17763
; 合规四落点之三:安装向导许可页。
LicenseFile={#RepoRoot}\LICENSE
; 安装器与快捷方式图标同源于随包资产。
SetupIconFile={#RepoRoot}\assets\app-icon.ico
UninstallDisplayIcon={app}\assets\app-icon.ico
OutputDir={#OutputDir}
#ifdef AcceptanceFixture
OutputBaseFilename=slTerminal-v{#AppVersion}-windows-{#Architecture}-acceptance-setup
#else
OutputBaseFilename=slTerminal-v{#AppVersion}-windows-{#Architecture}-setup
#endif
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "{#IslFile}"

[CustomMessages]
AutoStart=Start {#AppDisplayName} automatically
chinesesimplified.AutoStart=随 Windows 自动启动 {#AppDisplayName}

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "{cm:AutoStart}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BuildRoot}\slterm.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BuildRoot}\slterm-hook.exe"; DestDir: "{app}\runtime"; Flags: ignoreversion
Source: "{#RepoRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\README.zh-CN.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\CHANGELOG.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\app-icon.png"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\app-icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "{#RepoRoot}\docs\featureSpec\assets\cli-svg\*.svg"; DestDir: "{app}\assets\cli-svg"; Flags: ignoreversion
Source: "{#RepoRoot}\LICENSE"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\licenses\LICENSE-LATIN-MODERN-MATH"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\THIRD-PARTY-NOTICES"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\licenses\THIRD-PARTY-CRATE-NOTICES"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\licenses\LICENSE-DAV1D"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppDisplayName}"; Filename: "{app}\slterm.exe"; IconFilename: "{app}\assets\app-icon.ico"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "{#AppUserModelId}"
Name: "{group}\{cm:UninstallProgram,{#AppDisplayName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppDisplayName}"; Filename: "{app}\slterm.exe"; IconFilename: "{app}\assets\app-icon.ico"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "{#AppUserModelId}"; Tasks: desktopicon
Name: "{userstartup}\{#AppDisplayName}"; Filename: "{app}\slterm.exe"; IconFilename: "{app}\assets\app-icon.ico"; WorkingDir: "{%USERPROFILE}"; AppUserModelID: "{#AppUserModelId}"; Tasks: autostart

[Registry]
#ifdef AcceptanceFixture
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm-acceptance.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\slterm.exe"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm-acceptance.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"
#else
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\slterm.exe"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\slterm.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"
#endif

[Run]
Filename: "{app}\slterm.exe"; Description: "{cm:LaunchProgram,{#AppDisplayName}}"; WorkingDir: "{%USERPROFILE}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; 必须在 Inno 删除 slterm.exe 前让应用自己清理注入各 AI CLI 的 hook,
; 避免用户配置残留指向已删程序。夹具模式不得触碰真实 hook。
#ifndef AcceptanceFixture
Filename: "{app}\slterm.exe"; Parameters: "setup-ai --remove"; WorkingDir: "{app}"; RunOnceId: "RemoveSltermAiHooks"; Flags: runhidden skipifdoesntexist
#endif

[UninstallDelete]
; 更新交接锁与探测临时文件不在 [Files] 清单内;应用正常退出已尝试删除,
; 此处兜底异常退出的残留,避免安装目录因单文件残留而保留。
Type: files; Name: "{app}\.slterm-update*"
