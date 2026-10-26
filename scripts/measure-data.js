#!/usr/bin/env node
// A measurement, not a gate (docs/waffle.md section 4): what it costs a page to get the cancho-www list of projects from `data/ecosystem.json` at run time
// (what cancho-ui can do today) and as literals (what a build-time expansion would write), in Chromium and with the compiler's own commands.
//   node scripts/measure-data.js /path/to/cancho-www
'use strict';
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { chromium, devices } = require('playwright');

const www = path.resolve(process.argv[2] || '');
if (!fs.existsSync(path.join(www, 'data', 'ecosystem.json'))) { console.error('usage: node scripts/measure-data.js <path of a cancho-www clone>'); process.exit(2); }
const BIN = path.join(__dirname, '..', 'target', 'debug', 'cancho-ui');
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'data-'));
const data = JSON.parse(fs.readFileSync(path.join(www, 'data', 'ecosystem.json'), 'utf8'));
const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
const run = (args) => spawnSync(BIN, args, { encoding: 'utf8' });
function serve(dir) {
  return new Promise((ok) => {
    const s = http.createServer((q, r) => {
      let f = path.join(dir, decodeURIComponent(q.url.split('?')[0]));
      if (fs.existsSync(f) && fs.statSync(f).isDirectory()) { f = path.join(f, 'index.html'); }
      fs.readFile(f, (e, d) => { if (e) { r.statusCode = 404; r.end(); return; } r.setHeader('content-type', f.endsWith('.json') ? 'application/json' : f.endsWith('.html') ? 'text/html' : 'text/javascript'); r.end(d); });
    }).listen(0, () => ok(s));
  });
}
(async () => {
  // A. at run time: net.get + json.items, a Table row for each project
  const a = path.join(work, 'a'); fs.mkdirSync(path.join(a, 'data'), { recursive: true });
  fs.copyFileSync(path.join(www, 'data', 'ecosystem.json'), path.join(a, 'data', 'ecosystem.json'));
  fs.writeFileSync(path.join(a, 'p.cui'), `component Projects {
    requires Network;
    state r: Result[String, String] = Ok("");
    state body: String = "";
    state names: List[String] = [];
    on Load {
        r = await(net.get("data/ecosystem.json"));
        body = match r {
            Ok(text) => { text }
            Err(_) => { "" }
            Cancelled => { "" }
        };
        names = map(json.items(body, "projects"), p => json.get(p, "name", ""));
    }
    start Load;
    view { Column { Heading { level: 1 "Projects" } For { over: names as: n Text { "{n}" } } } }
}
`);
  const chk = run(['check', path.join(a, 'p.cui')]);
  const eff = run(['inspect', 'effects', path.join(a, 'p.cui'), 'Projects']);
  // B. as literals: one Text for each project
  const b = path.join(work, 'b'); fs.mkdirSync(b, { recursive: true });
  fs.writeFileSync(path.join(b, 'p.cui'), `component Projects {\n    view { Column { Heading { level: 1 "Projects" } ${data.projects.map((p) => `Text { ${JSON.stringify(p.name)} }`).join(' ')} } }\n}\n`);
  const effB = run(['inspect', 'effects', path.join(b, 'p.cui'), 'Projects']);
  console.log(`program with net.get: check exit ${chk.status} (${(chk.stderr || '').split('\n')[0].slice(0, 160)}); inspect effects requires ${JSON.stringify((JSON.parse(eff.stdout || '{}').requires) || null)}`);
  console.log(`program with the ${data.projects.length} names as literals: inspect effects requires ${JSON.stringify(JSON.parse(effB.stdout || '{}').requires)}`);
  const browser = await chromium.launch();
  for (const [name, dir, file] of [['at run time (net.get)', a, 'p.cui'], ['as literals', b, 'p.cui']]) {
    const out = path.join(work, 'out-' + name.replace(/\W/g, ''));
    const r = run(['build', path.join(dir, file), '--page', '-o', out]);
    if (r.status !== 0) { console.log(`${name}: build failed ${r.stderr.slice(0, 200)}`); continue; }
    if (dir === a) { fs.mkdirSync(path.join(out, 'data'), { recursive: true }); fs.copyFileSync(path.join(a, 'data', 'ecosystem.json'), path.join(out, 'data', 'ecosystem.json')); }
    const srv = await serve(out);
    const url = `http://127.0.0.1:${srv.address().port}/`;
    const html = await (await fetch(url)).text();
    const row = {};
    for (const js of [false, true]) {
      const ctx = await browser.newContext({ ...devices['Pixel 7'], javaScriptEnabled: js });
      const page = await ctx.newPage();
      const reqs = [];
      page.on('request', (q) => reqs.push(q.url().replace(url, '/')));
      await page.goto(url);
      await sleep(800);
      row[js ? 'js' : 'nojs'] = { names: await page.evaluate(() => (document.body.innerText.match(/cancho[\w-]*/g) || []).length), requests: reqs.length };
      await ctx.close();
    }
    console.log(`${name}: index.html ${html.length} bytes, app.js ${fs.statSync(path.join(out, 'app.js')).size} bytes; project names visible with script off ${row.nojs.names}, with script on ${row.js.names}; requests ${row.nojs.requests} (off), ${row.js.requests} (on: ${JSON.stringify(row.js.requests)})`);
    srv.close();
  }
  // C. what diff sees when the data changes and the program does not
  const changed = JSON.parse(JSON.stringify(data)); changed.projects[0].stage = 'alpha';
  fs.writeFileSync(path.join(a, 'data', 'ecosystem.json'), JSON.stringify(changed));
  const d = run(['diff', path.join(a, 'p.cui'), path.join(a, 'p.cui')]);
  console.log(`the data file changed (a stage), the program did not: diff exit ${d.status} (0 is "no semantic change"): ${(d.stdout.split('\n').find((l) => /change|unchanged/i.test(l)) || d.stdout.split('\n')[0] || '').trim()}`);
  await browser.close();
  fs.rmSync(work, { recursive: true, force: true });
})();
