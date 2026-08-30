import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// 字体（@fontsource 本地打包，桌面应用无 CDN 依赖）
import "@fontsource-variable/fraunces";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
// 设计令牌（Tailwind v4）
import "./styles/tokens.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
