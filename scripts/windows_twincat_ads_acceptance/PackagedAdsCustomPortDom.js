"use strict";

function expandAdvanced(evaluate) {
  return evaluate(
    ".react-flow",
    `var pane=d.querySelector('aside[aria-label="Discover devices"]');var toggle=pane?.querySelector('[data-role="ads-advanced-toggle"]');if(!toggle)return {clicked:false,reason:'advanced-toggle-missing'};if(toggle.getAttribute('aria-expanded')!=='true')toggle.click();return {clicked:true};`
  );
}

async function enterCustomPorts(evaluate, csv, sleep) {
  const deadline = Date.now() + 10_000;
  let result;
  while (Date.now() < deadline) {
    result = await evaluate(
      ".react-flow",
      `var input=d.querySelector('aside[aria-label="Discover devices"] [data-role="ads-custom-ports"]');if(!input)return {ready:false};var setter=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value')?.set;if(!setter)return {ready:false,reason:'native-setter-missing'};setter.call(input,${JSON.stringify(csv)});input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));return {ready:true,value:input.value,input_event_dispatched:true,change_event_dispatched:true};`
    );
    if (result?.ready && result.value === csv) return result;
    await sleep(50);
  }
  throw new Error(`Timed out entering custom ADS ports: ${result?.reason || "input missing"}.`);
}

function clickInnerRescan(evaluate) {
  return evaluate(
    ".react-flow",
    `var pane=d.querySelector('aside[aria-label="Discover devices"]');var button=pane?.querySelector('[data-role="ads-discover"]');if(!button)return {clicked:false,reason:'inner-scan-missing'};var text=(button.textContent||'').trim();if(text!=='Scan ADS again')return {clicked:false,reason:'unexpected-label',text:text};if(button.disabled)return {clicked:false,reason:'disabled',title:button.title||''};button.click();return {clicked:true,text:text};`
  );
}

module.exports = { clickInnerRescan, enterCustomPorts, expandAdvanced };
