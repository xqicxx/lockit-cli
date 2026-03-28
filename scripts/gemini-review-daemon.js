#!/usr/bin/env node
// Gemini Review Daemon — Node.js version
const crypto = require('crypto');
const https = require('https');
const fs = require('fs');

const APP_ID = '3208000';
const KEY_FILE = '/root/.config/gemini-review/key.pem';
const GEMINI_KEY = fs.readFileSync('/root/.config/gemini-review/gemini-api-key', 'utf8').trim();
const REPO = 'xqicxx/lockit';
const STATE_FILE = '/root/.config/gemini-review/last_reviewed.json';

function jwt() {
  const key = fs.readFileSync(KEY_FILE, 'utf8');
  const header = Buffer.from(JSON.stringify({alg:'RS256',typ:'JWT'})).toString('base64url');
  const payload = Buffer.from(JSON.stringify({iat:Math.floor(Date.now()/1000)-30,exp:Math.floor(Date.now()/1000)+570,iss:APP_ID})).toString('base64url');
  const sign = crypto.createSign('SHA256');
  sign.update(header+'.'+payload);
  const signature = sign.sign(key, 'base64url');
  return header+'.'+payload+'.'+signature;
}

async function api(path) {
  const j = jwt();
  return new Promise((resolve, reject) => {
    const req = https.request({hostname:'api.github.com',path,headers:{'Authorization':'Bearer '+j,'User-Agent':'node','Accept':'application/vnd.github.v3+json'}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>{ try{resolve(JSON.parse(d))}catch(e){reject(d)} });
    });
    req.end();
  });
}

async function apiPost(path, body) {
  const j = jwt();
  return new Promise((resolve, reject) => {
    const data = JSON.stringify(body);
    const req = https.request({hostname:'api.github.com',path,method:'POST',headers:{'Authorization':'Bearer '+j,'User-Agent':'node','Accept':'application/vnd.github.v3+json','Content-Type':'application/json','Content-Length':Buffer.byteLength(data)}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>{ try{resolve(JSON.parse(d))}catch(e){reject(d)} });
    });
    req.write(data);
    req.end();
  });
}

async function getInstallationToken() {
  const j = jwt();
  return new Promise((resolve, reject) => {
    const req = https.request({hostname:'api.github.com',path:'/app/installations/119617215/access_tokens',method:'POST',headers:{'Authorization':'Bearer '+j,'User-Agent':'node','Accept':'application/vnd.github.v3+json'}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>{
        try { resolve(JSON.parse(d).token); } catch(e) { reject(d); }
      });
    });
    req.end();
  });
}

async function callGemini(diff) {
  const body = JSON.stringify({
    contents: [{role:'user',parts:[{text:`你是 Rust + Svelte 全栈代码审查专家。审查以下 PR 代码变更。关注：安全性、代码质量、架构。输出中文简洁。每个问题标注 🔴严重/🟡建议/🟢可忽略。最后结论：可以Merge/需要修改。\n\n${diff}`}]}]
  });
  
  return new Promise((resolve, reject) => {
    const url = new URL(`https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent?key=${GEMINI_KEY}`);
    const req = https.request({hostname:url.hostname,path:url.pathname+url.search,method:'POST',headers:{'Content-Type':'application/json'}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>{
        try {
          const data = JSON.parse(d);
          const text = data.candidates?.[0]?.content?.parts?.[0]?.text || '审查失败';
          resolve(text);
        } catch(e) { resolve('审查解析失败: '+d.substring(0,200)); }
      });
    });
    req.write(body);
    req.end();
  });
}

async function getDiff(token, diffUrl) {
  return new Promise((resolve, reject) => {
    const req = https.request(new URL(diffUrl), {headers:{'Authorization':'token '+token,'Accept':'application/vnd.github.v3+json','User-Agent':'node'}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>resolve(d.substring(0,15000)));
    });
    req.end();
  });
}

async function postComment(token, number, body) {
  await new Promise((resolve, reject) => {
    const data = JSON.stringify({body});
    const req = https.request({hostname:'api.github.com',path:`/repos/${REPO}/issues/${number}/comments`,method:'POST',headers:{'Authorization':'token '+token,'User-Agent':'node','Content-Type':'application/json','Content-Length':Buffer.byteLength(data)}}, res => {
      let d=''; res.on('data',c=>d+=c); res.on('end',()=>resolve(d));
    });
    req.write(data);
    req.end();
  });
}

async function main() {
  console.log('🤖 Gemini Review Daemon started');
  console.log(`   Repo: ${REPO}`);
  console.log('   Checking every 5 minutes...\n');

  while (true) {
    try {
      const token = await getInstallationToken();
      console.log(`[${new Date().toLocaleString()}] Got token`);

      const prs = await new Promise((resolve, reject) => {
        const req = https.request({hostname:'api.github.com',path:`/repos/${REPO}/pulls?state=open&sort=updated&direction=desc&per_page=5`,method:'GET',headers:{'Authorization':'token '+token,'User-Agent':'node','Accept':'application/vnd.github.v3+json'}}, res => {
          let d=''; res.on('data',c=>d+=c); res.on('end',()=>{
            try { resolve(JSON.parse(d)); } catch(e) { reject(new Error('parse error')); }
          });
        });
        req.end();
      });

      let state = {};
      try { state = JSON.parse(fs.readFileSync(STATE_FILE, 'utf8')); } catch(e) {}

      for (const pr of prs) {
        const num = String(pr.number);
        const updated = pr.updated_at;
        if (state[num] === updated) continue;

        console.log(`  Reviewing PR #${pr.number}: ${pr.title}`);
        try {
          const diffUrl = pr.diff_url;
          const diff = await getDiff(token, diffUrl);
          if (!diff || diff.length < 50) { console.log('    Skip: no diff'); continue; }

          const review = await callGemini(diff);
          await postComment(token, pr.number, `## 🔍 Gemini Code Review\n\n${review}`);
          console.log(`    ✅ Posted`);
        } catch(e) {
          console.log(`    ❌ Error: ${e.message || e}`);
        }

        state[num] = updated;
        fs.writeFileSync(STATE_FILE, JSON.stringify(state));
      }
    } catch(e) {
      console.log(`❌ Loop error: ${e.message || e}`);
    }

    await new Promise(r => setTimeout(r, 300000)); // 5 min
  }
}

main();
