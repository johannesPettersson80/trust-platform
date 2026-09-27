import { pollValue } from "./plant-rendered-harness";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

/** Wait for the actual scroll viewport after asynchronous chart layout settles. */
export async function revealControlForCapture(inner: EvaluateWebview, selector: string): Promise<void> {
  await pollValue(`visible capture target ${selector}`, () => inner(`
    const target=d.querySelector(${JSON.stringify(selector)});
    if(!target)return 'missing';
    await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));
    target.scrollIntoView({block:'center',inline:'nearest'});
    await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));
    const r=target.getBoundingClientRect();let top=0,bottom=d.defaultView.innerHeight;
    for(let p=target.parentElement;p;p=p.parentElement){
      const s=d.defaultView.getComputedStyle(p);
      if(/auto|scroll|hidden|clip/.test(s.overflowY)){const clip=p.getBoundingClientRect();top=Math.max(top,clip.top);bottom=Math.min(bottom,clip.bottom);}
    }
    const hit=d.elementFromPoint(r.left+Math.min(r.width/2,150),r.top+r.height/2);
    const painted=hit===target||target.contains(hit);
    const monitor=d.querySelector('.monitor');
    return JSON.stringify({visible:r.width>0&&r.height>0&&r.top>=top&&r.bottom<=bottom&&painted,top:r.top,bottom:r.bottom,clipTop:top,clipBottom:bottom,hit:hit?.tagName,hitClass:hit?.className,rows:d.defaultView.getComputedStyle(d.querySelector('[data-control-studio]')).gridTemplateRows,monitor:monitor?.getBoundingClientRect().toJSON()});
  `), value => { try { return JSON.parse(value).visible; } catch { return false; } }, 5000);
}
