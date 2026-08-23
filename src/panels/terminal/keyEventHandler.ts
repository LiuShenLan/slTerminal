// 终端按键分发纯函数（TQ-E-02）——从 useXterm 内联体抽离，L3 headless 可挂。
import { getShortcutRegistry } from "../../features/shortcuts";

/** xterm attachCustomKeyEventHandler 回调：ShortcutRegistry 消费则拦截（返回 false），否则透传 */
export function handleTerminalKeyEvent(event: KeyboardEvent): boolean {
  if (event.type !== "keydown") return true;
  const consumed = getShortcutRegistry().resolve(event, "terminal");
  if (consumed) {
    event.preventDefault();
    return false;
  }
  return true;
}
