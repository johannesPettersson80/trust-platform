"use strict";

function selectOneVariable(evaluate) {
  return evaluate(
    ".react-flow",
    `var pane=d.querySelector('aside[aria-label="Browse variables"]');if(!pane)return {selected:false,reason:'pane-missing'};var allowWrites=pane.querySelector('[data-role="allow-writes"]');if(!allowWrites)return {selected:false,reason:'write-toggle-missing'};if(allowWrites.checked)return {selected:false,reason:'writes-enabled'};var choices=[...pane.querySelectorAll('input[aria-label^="Select "]')].filter(function(input){return !input.disabled;}).map(function(input){var text=(input.closest('[data-role="symbol-leaf"]')?.textContent||'').replace(/\\s+/g,' ').trim();return {input:input,text:text,readOnly:/read-only/i.test(text),nonStringScalar:/\\b(?:BOOL|SINT|INT|DINT|LINT|USINT|UINT|UDINT|ULINT|REAL|LREAL|BYTE|WORD|DWORD|LWORD)\\b/i.test(text)&&!/\\bSTRING\\b/i.test(text)};});var preferred=choices.find(function(choice){return choice.readOnly&&choice.nonStringScalar;});var choice=preferred||choices.find(function(candidate){return candidate.readOnly;})||choices[0];if(!choice)return {selected:false,reason:'variable-missing'};var checkbox=choice.input;if(!checkbox.checked)checkbox.click();var type=/\\b(BOOL|SINT|INT|DINT|LINT|USINT|UINT|UDINT|ULINT|REAL|LREAL|BYTE|WORD|DWORD|LWORD|STRING)\\b/i.exec(choice.text)?.[1]||'';return {selected:true,remote_symbol:(checkbox.getAttribute('aria-label')||'').replace(/^Select /,''),row_text:choice.text,selected_type:type.toUpperCase(),selection_preference:preferred?'read-only-non-string-scalar':choice.readOnly?'read-only-fallback':'first-selectable-fallback',allow_writes:false,configured_access:'read'};`
  );
}

function addSelectedVariable(evaluate) {
  return evaluate(
    ".react-flow",
    `var pane=d.querySelector('aside[aria-label="Browse variables"]');var allowWrites=pane?.querySelector('[data-role="allow-writes"]');if(!pane||!allowWrites)return {clicked:false,reason:'pane-missing'};if(allowWrites.checked)return {clicked:false,reason:'writes-enabled'};var add=[...pane.querySelectorAll('button')].find(function(button){return /^Add variables \\(1\\)$/.test((button.textContent||'').trim());});if(!add)return {clicked:false,reason:'add-missing'};if(add.disabled)return {clicked:false,reason:'add-disabled',title:add.title||''};add.click();return {clicked:true,allow_writes:false,configured_access:'read'};`
  );
}

module.exports = { addSelectedVariable, selectOneVariable };
