let handle;

export function start(frame) {
  handle = window.requestAnimationFrame(() => {
    start(frame);
    frame();
  });
}

export function stop() {
  if (typeof handle !== "undefined") {
    window.cancelAnimationFrame(handle);
  }
}