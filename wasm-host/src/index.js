export function redirect_print(out, err) {
  out_print = out;
  err_print = err;
}

export let out_print = console.log;
export let err_print = console.error;
