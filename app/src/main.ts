import { mount } from "svelte";
import App from "./App.svelte";
import DiagnosticsWindow from "./lib/components/DiagnosticsWindow.svelte";
import "./app.css";

// The Diagnostics window loads the same page with #diagnostics.
const Root = location.hash === "#diagnostics" ? DiagnosticsWindow : App;

export default mount(Root, { target: document.getElementById("app")! });
