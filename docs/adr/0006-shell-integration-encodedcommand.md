# Shell 集成通过 EncodedCommand 内联注入（不写磁盘）

Status: Accepted

PowerShell shell 集成脚本编译时嵌入二进制（`include_str!`），启动时通过 `-EncodedCommand`（UTF-16LE Base64）内联传递给 PowerShell，不写任何文件到磁盘。

**为什么？** 传统终端写入 `%APPDATA%\Microsoft\Windows\PowerShell\profile.ps1` 会触发 AMSI（Anti-Malware Scan Interface）和 ASR（Attack Surface Reduction）规则——Windows Defender 可能将修改 profile 文件的未知进程标记为可疑行为。内联注入完全消除磁盘写入，零触发风险。

**备选方案**：`-File` 临时脚本（仍需写磁盘）；`-Command` 明文参数（长度限制和转义问题）；不注入集成脚本（cwd 和提示符边界解析不可靠）。

**后果**：脚本内容编译时固定，修改集成需重新编译。仅适用于 PowerShell（cmd.exe 无等效机制）。
