// 所有构建模式均隐藏 Windows 控制台窗口
#![windows_subsystem = "windows"]

fn main() {
    // panic hook：将 panic 信息写入文件以便诊断
    std::panic::set_hook(Box::new(|info| {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| ".".into())
            .join("crash.log");
        if let Ok(mut f) = std::fs::File::create(&path) {
            use std::io::Write;
            let _ = writeln!(f, "PANIC: {:?}", info);
        } else {
            eprintln!("PANIC: {:?}", info);
        }
    }));
    slterminal_lib::run()
}
