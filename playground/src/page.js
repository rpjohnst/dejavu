export function clear(output) {
  output.innerHTML = "";
}

export function outPrint(output, string) {
  const span = document.createElement("span");
  span.innerText = string;
  output.appendChild(span);
  span.scrollIntoView();
}

export function errPrint(output, string) {
  const span = document.createElement("span");
  span.className = "error";
  span.innerText = string;
  output.appendChild(span);
  span.scrollIntoView();
}
