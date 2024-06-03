"use strict";
const { invoke } = window.__TAURI__.tauri;

const { listen } = window.__TAURI__.event

import { SetUpSpectrumInterface } from "./stitch-assets/script.js";
// const controller = new AbortController();

listen('tauri://file-drop', event => {
  document.querySelector("html").classList.remove("file-drop-hover");
  for (let i = 0; i < event.payload.length; i++) {
    let file = event.payload[i];
    if (file.toLowerCase().endsWith(".mgf") || file.toLowerCase().endsWith(".mgf.gz")) {
      load_mgf(file);
    } else {
      load_identified_peptides(file);
    }
  }
})

listen('tauri://file-drop-hover', event => {
  document.querySelector("html").classList.add("file-drop-hover");
})

listen('tauri://file-drop-cancelled', event => {
  document.querySelector("html").classList.remove("file-drop-hover");
})

// function abort() {
//   console.log(controller);
//   controller.abort("User manual abort");
// }

/**
* @param e: Element
*/
async function select_mgf_file(e) {
  let properties = {
    //defaultPath: 'C:\\',
    directory: false,
    filters: [{
      extensions: ['mgf', 'mgf.gz'], name: "*"
    }]
  };
  window.__TAURI__.dialog.open(properties).then((result) => {
    if (result != null) {
      e.dataset.filepath = result;
      load_mgf(e.dataset.filepath);
    }
  })
};

/**
* @param e: Element
*/
async function select_identified_peptides_file(e) {
  let properties = {
    //defaultPath: 'C:\\',
    directory: false,
    filters: [{
      extensions: ["csv", "csv.gz", "tsv", "tsv.gz", "psmtsv", "psmtsv.gz", "fasta", "fasta.gz"], name: "*"
    }]
  };
  window.__TAURI__.dialog.open(properties).then((result) => {
    if (result != null) {
      e.dataset.filepath = result;
      load_identified_peptides(e.dataset.filepath);
    }
  })
};

async function load_mgf(path) {
  document.querySelector("#load-mgf-path").classList.add("loading")
  invoke("load_mgf", { path: path }).then((result) => {
    document.querySelector("#spectrum-log").innerText = "Loaded " + result + " spectra";
    document.querySelector("#loaded-path").classList.remove("error");
    document.querySelector("#loaded-path").innerText = path.split('\\').pop().split('/').pop();
    document.querySelector("#number-of-scans").innerText = result;
    spectrum_details();
    document.querySelector("#load-mgf-path").classList.remove("loading")
  }).catch((error) => {
    document.querySelector("#loaded-path").classList.add("error");
    document.querySelector("#loaded-path").innerHTML = showError(error)
    document.querySelector("#load-mgf-path").classList.remove("loading")
  })
}

async function load_identified_peptides(path) {
  document.querySelector("#load-identified-peptides").classList.add("loading")
  invoke("load_identified_peptides", { path: path }).then((result) => {
    document.querySelector("#identified-peptides-log").innerText = "Loaded " + result + " peptides";
    document.querySelector("#loaded-identified-peptides-path").classList.remove("error");
    document.querySelector("#loaded-identified-peptides-path").innerText = path.split('\\').pop().split('/').pop();
    document.querySelector("#number-of-identified-peptides").innerText = result;
    document.querySelector("#peptides").style.display = "block";
    displayed_identified_peptide = undefined;
    identified_peptide_details();
    document.querySelector("#load-identified-peptides").classList.remove("loading")
  }).catch((error) => {
    document.querySelector("#loaded-identified-peptides-path").classList.add("error");
    document.querySelector("#loaded-identified-peptides-path").innerHTML = showError(error)
    document.querySelector("#load-identified-peptides").classList.remove("loading")
  })
}

async function load_clipboard() {
  document.querySelector("#load-clipboard").classList.add("loading");
  navigator.clipboard
    .readText()
    .then(async (clipText) => {
      invoke("load_clipboard", { data: clipText }).then((result) => {
        document.querySelector("#spectrum-log").innerText = "Loaded " + result + " spectra";
        document.querySelector("#loaded-path").classList.remove("error");
        document.querySelector("#loaded-path").innerText = "Clipboard";
        document.querySelector("#number-of-scans").innerText = result;
        spectrum_details();
        document.querySelector("#load-clipboard").classList.remove("loading")
      }).catch((error) => {
        document.querySelector("#loaded-path").classList.add("error");
        document.querySelector("#loaded-path").innerHTML = showError(error);
        document.querySelector("#load-clipboard").classList.remove("loading")
      })
    })
    .catch(() => {
      document.querySelector("#loaded-path").classList.add("error");
      document.querySelector("#loaded-path").innerHTML = showError("Could not load clipboard, did you give permission to read the clipboard?");
      document.querySelector("#load-clipboard").classList.remove("loading")
    });
}

async function find_scan_number() {
  invoke("find_scan_number", { scanNumber: Number(document.querySelector("#scan-number").value) }).then(
    (result) => {
      document.querySelector("#details-spectrum-index").value = result;
      spectrum_details();
    }
  ).catch((error) => {
    document.querySelector("#spectrum-error").classList.remove("hidden");
    document.querySelector("#spectrum-error").innerHTML = showError(error);
  })
}

async function spectrum_details() {
  invoke("spectrum_details", { index: Number(document.querySelector("#details-spectrum-index").value) }).then(
    (result) => document.querySelector("#spectrum-details").innerText = result
  ).catch((error) => {
    document.querySelector("#spectrum-error").classList.remove("hidden");
    document.querySelector("#spectrum-error").innerHTML = showError(error);
    document.querySelector("#spectrum-details").innerText = "ERROR";
  })
}

let displayed_identified_peptide = undefined;
async function identified_peptide_details() {
  let index = Number(document.querySelector("#details-identified-peptide-index").value);
  if (displayed_identified_peptide != index) {
    invoke("identified_peptide_details", { index: index }).then((result) => {
      document.querySelector("#identified-peptide-details").innerHTML = result;
      displayed_identified_peptide = index;
    }).catch((error) => {
      document.querySelector("#spectrum-error").classList.remove("hidden");
      document.querySelector("#spectrum-error").innerHTML = showError(error);
      document.querySelector("#identified-peptide-details").innerText = "ERROR";
    })
  }
}

async function search_peptide() {
  if (document.querySelector("#search-peptide-input").value != "") {
    document.querySelector("#search-peptide").classList.add("loading");
    invoke("search_peptide", { text: document.querySelector("#search-peptide-input").value, minimalMatchScore: Number(document.querySelector("#search-peptide-minimal-match").value), minimalPeptideScore: Number(document.querySelector("#search-peptide-minimal-peptide").value) }).then((result) => {
      document.querySelector("#resulting-peptides").innerHTML = result;
      document.querySelector("#search-peptide").classList.remove("loading");
    }).catch((error) => {
      document.querySelector("#spectrum-error").classList.remove("hidden");
      document.querySelector("#spectrum-error").innerHTML = showError(error);
      document.querySelector("#search-peptide").classList.remove("loading");
      document.querySelector("#resulting-peptides").innerText = "ERROR";
    })
  } else {
    document.querySelector("#resulting-peptides").innerText = "";
  }
}

async function search_modification() {
  if (document.querySelector("#search-modification").value != "") {
    document.querySelector("#search-modification-button").classList.add("loading");
    invoke("search_modification", { text: document.querySelector("#search-modification").value, tolerance: Number(document.querySelector("#search-modification-tolerance").value) }).then((result) => {
      document.querySelector("#search-modification-result").innerHTML = result;
      document.querySelector("#search-modification-button").classList.remove("loading");
      document.querySelector("#search-modification-error").classList.add("hidden");
    }).catch((error) => {
      document.querySelector("#search-modification-error").classList.remove("hidden");
      document.querySelector("#search-modification-error").innerHTML = showError(error);
      document.querySelector("#search-modification-button").classList.remove("loading");
    })
  } else {
    document.querySelector("#resulting-modification-result").innerText = "";
  }
}

async function details_formula(event) {
  invoke("details_formula", { text: event.target.value }).then((result) => {
    document.querySelector("#details-formula-result").innerHTML = result;
    document.querySelector("#details-formula-error").classList.add("hidden");
  }).catch((error) => {
    document.querySelector("#details-formula-error").classList.remove("hidden");
    document.querySelector("#details-formula-error").innerHTML = showError(error);
  })
}

async function load_identified_peptide() {
  let index = Number(document.querySelector("#details-identified-peptide-index").value);
  invoke("load_identified_peptide", { index: index }).then((result) => {
    document.querySelector("#peptide").innerText = result.peptide;
    document.querySelector("#spectrum-charge").value = result.charge;
    document.querySelector("#spectrum-model").value = result.mode.toLowerCase();
    document.querySelector("#details-spectrum-index").value = result.scan_index;
  }).catch((error) => {
    document.querySelector("#spectrum-error").classList.remove("hidden");
    document.querySelector("#spectrum-error").innerHTML = showError(error);
    document.querySelector("#identified-peptide-details").innerText = "ERROR";
  })
}


function get_location(id) {
  let loc = document.querySelector(id);
  let t = loc.children[0].options[Number(loc.children[0].value)].dataset.value;
  // [[{\"SkipN\":1},[\"Water\"]],[\"All\",[\"Water\"]],[{\"SkipNC\":[1,2]},[\"Water\"]],[{\"TakeN\":{\"skip\":2,\"take\":1}},[\"Water\"]]]
  if (["SkipN", "SkipC", "TakeC"].includes(t)) {
    let obj = {};
    obj[t] = Number(loc.children[1].value);
    return obj;
  } else if (t == "SkipNC") {
    let obj = {};
    obj[t] = [Number(loc.children[1].value), Number(loc.children[2].value)];
    return obj;
  } else if (t == "TakeN") {
    return { t: { "skip": Number(loc.children[1].value), "take": Number(loc.children[2].value) } };
  } else {
    return t;
  }
}

function get_noise_filter(id) {
  let loc = document.querySelector(id);
  let t = loc.children[0].options[Number(loc.children[0].value)].dataset.value;
  if (["Relative", "Absolute"].includes(t)) {
    let obj = {};
    obj[t] = Number(loc.children[1].value);
    return obj;
  } else if (t == "TopX") {
    let obj = {};
    obj[t] = [Number(loc.children[1].value), Number(loc.children[2].value)];
    return obj;
  } else {
    return t;
  }
}

function get_losses(ion) {
  let loss = ""
  document.getElementsByName("model-" + ion + "-loss-selection").forEach(element => {
    if (element.checked) {
      if (loss == "") {
        loss = element.value;
      } else {
        loss += "," + element.value;
      }
    }
  });
  document.querySelectorAll("#model-" + ion + "-loss>*").forEach(child => {
    if (child.classList.contains("element")) { // TODO: handle numeric neutral losses + potentially make the handling of the separate ones somewhat better, give a list or something
      let custom = "-" + child.dataset.value;
      if (loss == "") {
        loss = custom;
      } else {
        loss += "," + custom;
      }
    }
  });
  return loss;
}

//import { SpectrumSetUp } from "./stitch-assets/script.js";
async function annotate_spectrum() {
  document.querySelector("#annotate-button").classList.add("loading");
  document.querySelector("#peptide").innerText = document.querySelector("#peptide").innerText.trim();
  var charge = document.querySelector("#spectrum-charge").value == "" ? null : Number(document.querySelector("#spectrum-charge").value);
  var noise_threshold = get_noise_filter("#noise-filter");
  var model = {
    a: [get_location("#model-a-location"), get_losses("a")],
    b: [get_location("#model-b-location"), get_losses("b")],
    c: [get_location("#model-c-location"), get_losses("c")],
    d: [get_location("#model-d-location"), get_losses("d")],
    v: [get_location("#model-v-location"), get_losses("v")],
    w: [get_location("#model-w-location"), get_losses("w")],
    x: [get_location("#model-x-location"), get_losses("x")],
    y: [get_location("#model-y-location"), get_losses("y")],
    z: [get_location("#model-z-location"), get_losses("z")],
    precursor: get_losses("precursor"),
    immonium: document.querySelector("#model-immonium-enabled").checked,
    m: document.querySelector("#model-m-enabled").checked,
    modification_neutral: document.querySelector("#model-modification-neutral-enabled").checked,
    modification_diagnostic: document.querySelector("#model-modification-diagnostic-enabled").checked,
    glycan: [document.querySelector("#model-glycan-enabled").checked, get_losses("glycan")],
  };
  invoke("annotate_spectrum", { index: Number(document.querySelector("#details-spectrum-index").value), tolerance: [Number(document.querySelector("#spectrum-tolerance").value), document.querySelector("#spectrum-tolerance-unit").value], charge: charge, filter: noise_threshold, model: document.querySelector("#spectrum-model").value, peptide: document.querySelector("#peptide").innerText, customModel: model, massMode: document.querySelector("#spectrum-mass-mode").value }).then((result) => {
    document.querySelector("#spectrum-results-wrapper").innerHTML = result.spectrum;
    document.querySelector("#spectrum-fragment-table").innerHTML = result.fragment_table;
    document.querySelector("#spectrum-log").innerText = result.log;
    document.querySelector("#spectrum-error").innerText = "";
    document.querySelector("#spectrum-wrapper").classList.remove("hidden"); // Remove hidden class if this is the first run
    document.querySelector("#spectrum-error").classList.add("hidden");
    document.querySelector("#spectrum-mz-max").value = result.mz_max;
    document.querySelector("#spectrum-intensity-max").value = result.intensity_max;
    document.querySelector("#spectrum-label").value = 90;
    document.querySelector("#spectrum-label-value").value = 90;
    document.querySelector("#spectrum-masses").value = 0;
    document.querySelector("#spectrum-masses-value").value = 0;
    SetUpSpectrumInterface();
    document.querySelector("#annotate-button").classList.remove("loading");
  }).catch((error) => {
    document.querySelector("#spectrum-error").classList.remove("hidden");
    document.querySelector("#spectrum-error").innerHTML = showError(error, false);
    document.querySelector("#peptide").innerHTML = showContext(error, document.querySelector("#peptide").innerText);
    document.querySelector("#annotate-button").classList.remove("loading");
  })
}

function showError(error, showContext = true) {
  console.error(error);
  if (typeof error == "string") {
    return "<div class='raw'>" + error + "</div>";
  } else {
    let msg = "<p class='title'>" + error.short_description + "</p><p class='description'>" + error.long_description + "</p>";
    if (showContext) {
      if (error.context.hasOwnProperty('Line')) {
        let Line = error.context.Line;
        msg += "<pre>" + Line.line + "\n" + " ".repeat(Line.offset) + "^".repeat(Line.length) + "</pre>";
      } else {
        msg += "<pre>" + error.context + "</pre>";
      }
    }
    if (error.suggestions.length > 0) {
      msg += "<p>Did you mean any of the following?</p><ul>";
      for (let suggestion in error.suggestions) {
        msg += "<li>" + error.suggestions[suggestion] + "</li>";
      }
      msg += "</ul>";
    }
    return msg;
  }
}

function showContext(error, fallback) {
  if (error.context.hasOwnProperty('Line')) {
    let Line = error.context.Line;
    return Line.line.slice(0, Line.offset) + "<span class='error'>" + Line.line.slice(Line.offset, Line.offset + Line.length) + "</span>" + Line.line.slice(Line.offset + Line.length, Line.line.length);
  } else if (error.context.hasOwnProperty('FullLine')) {
    let FullLine = error.context.FullLine;
    return "<span class='error'>" + FullLine.line + "</span>";
  } else if (error.context = "None") {
    return fallback;
  } else {
    console.error("Error type not handled", error);
    return fallback;
  }
}

/** @param e {MouseEvent}  */
function resizeDown(e) {
  document.querySelector(".resize-wrapper").classList.add("active");
  document.querySelector(".resize-wrapper").dataset.start_x = e.clientX;
  document.querySelector(".resize-wrapper").dataset.left_width = document.querySelector(".resize-wrapper").firstElementChild.getBoundingClientRect().width - 16;
  document.addEventListener("mousemove", resizeMove);
  document.addEventListener("mouseup", resizeUp);
  document.querySelector(".resize-wrapper").style.userSelect = 'none';
}

/** @param e {MouseEvent}  */
function resizeMove(e) {
  let wrapper = document.querySelector(".resize-wrapper");
  let first = wrapper.firstElementChild;
  first.style.width =
    Math.max(10, Math.min(90, (Number(wrapper.dataset.left_width) + (e.clientX - Number(wrapper.dataset.start_x))) /
      wrapper.getBoundingClientRect().width * 100)) + "%";
  e.preventDefault();
}

function resizeUp() {
  document.querySelector(".resize-wrapper").classList.remove("active");
  document.removeEventListener("mousemove", resizeMove);
  document.removeEventListener("mouseup", resizeUp);
}

// Setup
window.addEventListener("DOMContentLoaded", () => {
  document.querySelector(".resize").addEventListener("mousedown", resizeDown);
  document.querySelectorAll(".collapsible>legend").forEach(element => element.addEventListener("click", (e) => document.getElementById(e.target.parentElement.dataset.linkedItem).toggleAttribute("checked")));
  document
    .querySelector("#load-mgf-path")
    .addEventListener("click", (event) => select_mgf_file(event.target));
  document
    .querySelector("#load-identified-peptides")
    .addEventListener("click", (event) => select_identified_peptides_file(event.target));
  document
    .querySelector("#load-identified-peptide")
    .addEventListener("click", (event) => load_identified_peptide());
  document
    .querySelector("#load-clipboard")
    .addEventListener("click", () => load_clipboard());
  document
    .querySelector("#search-peptide", search_peptide)
    .addEventListener("click", () => search_peptide());
  document
    .querySelector("#search-modification-button")
    .addEventListener("click", () => search_modification());
  document
    .querySelector("#details-formula")
    .addEventListener("input", details_formula);
  enter_event("#search-peptide-input", search_peptide)
  enter_event("#search-modification", search_modification)
  enter_event("#scan-number", find_scan_number)
  add_event("#details-spectrum-index", ["change", "focus"], spectrum_details)
  add_event("#details-identified-peptide-index", ["change", "focus"], identified_peptide_details)
  document
    .querySelector("#annotate-button")
    .addEventListener("click", () => annotate_spectrum());
  document
    .querySelector("#peptide")
    .addEventListener("focus", (event) => {
      event.target.innerHTML = event.target.innerText;
    });
  enter_event("#peptide", annotate_spectrum)

  // Set up all separated inputs
  document.querySelectorAll(".separated-input").forEach(t => {
    t.addEventListener("click", event => {
      if (event.target.classList.contains("separated-input")) {
        event.target.querySelector(".input").focus({ focusVisible: true })
        event.preventDefault();
      }
    });
  });
  document.querySelectorAll(".separated-input .input").forEach(t => {
    t.addEventListener("keydown", async event => {
      let input = event.target;
      let values_container = input.parentElement;
      let outer = input.parentElement.parentElement;
      outer.classList.toggle("error", false);
      if (event.keyCode == 13 || event.keyCode == 9) { // Enter or Tab
        let value = undefined;
        switch (input.dataset.type) {
          case "molecular_formula":
            value = await invoke("validate_molecular_formula", { text: input.innerText })
              .catch(error => {
                input.innerHTML = showContext(error, input.innerText);
                outer.querySelector("output.error").innerHTML = showError(error, false);
              });
            break;
          case "text":
            value = input.innerText.trim();
            break;
          default: console.error("Invalid separated input type");
        }
        if (value !== undefined) {
          let new_element = document.createElement("span");
          new_element.classList.add("element");
          new_element.innerHTML = value;
          new_element.dataset.value = new_element.innerText;
          new_element.addEventListener("click", e => {
            let input = e.target.parentElement.querySelector(".input");
            input.innerText = e.target.innerText.slice(0, -1);
            moveCursorToEnd(input);
            e.target.remove();
          });
          new_element.title = "Edit";
          let delete_button = document.createElement("button");
          delete_button.classList.add("delete");
          delete_button.appendChild(document.createTextNode("x"));
          delete_button.addEventListener("click", e => e.target.parentElement.remove());
          delete_button.title = "Delete";
          new_element.appendChild(delete_button);

          values_container.insertBefore(new_element, input);
          input.innerText = "";
        } else {
          console.log("Verification failed");
          outer.classList.toggle("error", true);
        }
        event.preventDefault();
      } else if (event.keyCode == 8 && !input.hasChildNodes()) { // Backspace
        if (values_container.children.length >= 3) {
          let target = values_container.children[values_container.children.length - 3];
          input.innerText = target.innerText.slice(0, -1);
          target.remove();
          moveCursorToEnd(input);
          event.preventDefault();
        }
      }
    });
  });
  document.querySelectorAll(".separated-input .clear").forEach(t => {
    t.addEventListener("click", e => {
      e.target.parentElement.querySelectorAll(".element").forEach(c => c.remove());
      e.target.parentElement.querySelector(".input").innerText = "";
      e.target.parentElement.parentElement.classList.remove("error");
      e.target.parentElement.parentElement.querySelector(".error").innerText = "";
    })
  });

  // Refresh interface for hot reload
  invoke("refresh").then((result) => {
    document.querySelector("#number-of-scans").innerText = result[0];
    if (result[0] > 0) {
      spectrum_details();
    }
    document.querySelector("#number-of-identified-peptides").innerText = result[1];
    if (result[1] > 0) {
      document.querySelector("#peptides").style.display = "block";
      identified_peptide_details();
    }
  })
});

function moveCursorToEnd(contentEle) {
  const range = document.createRange();
  const selection = window.getSelection();
  range.setStart(contentEle, contentEle.childNodes.length);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
};

function add_event(selector, events, callback) {
  for (let i = 0; i < events.length; i++) {
    document.querySelector(selector).addEventListener(events[i], callback);
  }
}

function enter_event(selector, callback) {
  document
    .querySelector(selector)
    .addEventListener("keydown", event => { if (event.keyCode == 13) { callback(event); event.preventDefault(); } else { } });
}