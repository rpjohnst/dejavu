const output = document.getElementById("output");

export function clear() {
  output.innerHTML = "";
}

export function out_print(string) {
  const span = document.createElement("span");
  span.innerText = string;
  output.appendChild(span);
}

export function err_print(string) {
  const span = document.createElement("span");
  span.className = "error";
  span.innerText = string;
  output.appendChild(span);
}
