export function schedule(frame) {
  return window.requestAnimationFrame(frame);
}

export function cancel(handle) {
  window.cancelAnimationFrame(handle);
}