/**
 * 标题阈值拖拽（PATCH001.2，移植 CapsuleTODO `useThresholdDrag`）：标题既是窗口拖动把手，
 * 又是"双击缩回托盘"的点击目标。全局 mousedown 的"按下即启动系统拖拽"会异步进入拖拽循环、
 * 吞掉后续 click（CT 2026-09-30 实测）——阈值分流：按下只记起点，位移超阈值才启动拖拽，
 * 单击/双击照常派发。返回显式 destroy：调用点在 onMounted 内，由持有方在 onUnmounted 显式调用。
 */
import { getCurrentInstance, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 拖拽启动阈值（px）：位移低于此视为点击，不启动拖拽 */
const DRAG_THRESHOLD = 5;

/** 给把手元素装上阈值拖拽（左键限定；`preventDefault` 仅抑制文本选择，不拦 click） */
export function useThresholdDrag(handle: HTMLElement): () => void {
  let startX = 0;
  let startY = 0;
  let tracking = false;

  /** 位移超阈值 → 启动系统拖拽并解绑（本次手势只触发一次） */
  const onMove = (e: MouseEvent): void => {
    if (!tracking) {
      return;
    }
    if (Math.hypot(e.clientX - startX, e.clientY - startY) < DRAG_THRESHOLD) {
      return;
    }
    tracking = false;
    detach();
    void getCurrentWindow()
      .startDragging()
      .catch((err) => console.error("窗口拖拽启动失败", err));
  };

  /** 抬起即结束本次手势（未超阈值 = 点击，click 照常派发） */
  const onUp = (): void => {
    tracking = false;
    detach();
  };

  /** 按下只记起点 */
  const onDown = (e: MouseEvent): void => {
    if (e.button !== 0) {
      return;
    }
    e.preventDefault();
    startX = e.clientX;
    startY = e.clientY;
    tracking = true;
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  /** 解绑本次手势的临时监听 */
  const detach = (): void => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };

  /** 卸载：摘把手监听 + 收尾未决手势 */
  const destroy = (): void => {
    handle.removeEventListener("mousedown", onDown);
    detach();
  };

  handle.addEventListener("mousedown", onDown);
  if (getCurrentInstance()) {
    onUnmounted(destroy); // 仅 setup 同步上下文可注册；异步上下文由持有方显式调用
  }
  return destroy;
}
