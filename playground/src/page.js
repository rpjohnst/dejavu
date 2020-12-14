export const canvas = document.getElementById("canvas");

const output = document.getElementById("output");

export function clear() {
  output.innerHTML = "";
}

export function out_print(string) {
  const span = document.createElement("span");
  span.innerText = string;
  output.appendChild(span);
  span.scrollIntoView();
}

export function err_print(string) {
  const span = document.createElement("span");
  span.className = "error";
  span.innerText = string;
  output.appendChild(span);
  span.scrollIntoView();
}
