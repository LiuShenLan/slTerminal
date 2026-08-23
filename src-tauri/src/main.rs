// 所有构建模式均隐藏 Windows 控制台窗口
#![windows_subsystem = "windows"]

fn main() {
    slterminal_lib::install_panic_hook();
    slterminal_lib::run()
}
