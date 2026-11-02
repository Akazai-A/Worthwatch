import { mount } from "svelte";
import "@fontsource/instrument-sans/400.css";
import "@fontsource/instrument-sans/500.css";
import "@fontsource/instrument-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "uplot/dist/uPlot.min.css";
import "./tokens.css";
import App from "./App.svelte";

export default mount(App, { target: document.getElementById("app")! });
