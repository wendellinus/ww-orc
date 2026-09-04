import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// 打包后禁止打开开发者工具：仅生产构建拦截 DevTools 快捷键（开发环境保留）
if (import.meta.env.PROD) {
  document.addEventListener("keydown", (e) => {
    const key = e.key.toUpperCase();
    if (e.key === "F12") {
      e.preventDefault();
    } else if (
      e.ctrlKey &&
      e.shiftKey &&
      (key === "I" || key === "J" || key === "C")
    ) {
      e.preventDefault();
    }
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
