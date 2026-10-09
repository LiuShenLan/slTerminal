//! slterm-hook:AI-CLI 生命周期 hook 小进程(hook 层,零生产依赖契约)。
//! 薄壳:行为合同与全部逻辑在 lib(库态可测),此处只做进程入口转发。

fn main() {
    slterm_hook::main();
}
