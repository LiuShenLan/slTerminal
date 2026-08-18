export { ErrorBoundary } from "./ErrorBoundary";
export { confirmDialog, ConfirmDialogHost, _resetConfirmDialog, type ConfirmDialogOptions } from "./ConfirmDialog";
export { toast, ToastHost, type ToastType } from "./toast";
export { normalizePath, basename, isChildOf, relativePath } from "./path";
export { createActivePointer } from "./activePointer";
export { useFontSizeWheel } from "./useFontSizeWheel";
export { E2E_ENABLED, computeE2eEnabled } from "./e2eEnabled";
export { injectScript } from "./injectScript";
export { type AgentStatus } from "./agentStatus";
export { parseTerminalPageId } from "./panelId";
export {
  parseAppError,
  getErrorMessage,
  APP_ERROR_VARIANTS,
  type ParsedAppError,
} from "../ipc/appError";
export {
  IconNav,
  IconFiles,
  IconCommit,
  IconConfig,
  IconChevronRight,
  IconChevronDown,
  IconRefresh,
  IconSearch,
  IconHistory,
  IconClose,
  IconMin,
  IconMax,
  IconCloseWin,
  IconPlus,
  IconFolder,
  IconEmptyBox,
  IconAlertTriangle,
  type IconProps,
} from "./icons";
export { StatusDot, type StatusDotStatus, type StatusDotProps } from "./StatusDot";
