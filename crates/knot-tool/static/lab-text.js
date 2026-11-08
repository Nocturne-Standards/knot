// Loads the vendored app-core text helpers for app.js, which is a classic script.
import { displayText } from "/vendor/app-core/text.js";
import { NAME_HIDES, looksAlike, nameHides, sameName } from "/vendor/app-core/names.js";

window.knotText = { displayText, NAME_HIDES, looksAlike, nameHides, sameName };
