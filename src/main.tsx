import { createRoot } from "react-dom/client";
import "@fontsource-variable/atkinson-hyperlegible-next";
import "@fontsource-variable/literata";
import "katex/dist/katex.min.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/layout.css";
import "./styles/chat.css";
import "./styles/renderers.css";
import "./styles/views.css";
import App from "./App";

createRoot(document.getElementById("root")!).render(<App />);
