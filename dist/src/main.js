// WebView 无打包器，走 withGlobalTauri 注入的全局 API
const invoke = window.__TAURI__.core.invoke;

let editingName = null; // null=添加；字符串=编辑
let menuTarget = null;  // ⋯ 菜单当前指向的服务名
let toastTimer = null;

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}

function toast(msg) {
  const el = document.querySelector('.toast') || (() => {
    const d = document.createElement('div');
    d.className = 'toast';
    document.body.appendChild(d);
    return d;
  })();
  el.textContent = msg;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 2000);
}

async function loadServices() {
  const container = document.getElementById('service-list');
  let services;
  try {
    services = await invoke('list_services');
  } catch (e) {
    container.innerHTML = '<div class="empty">加载失败：' + escapeHtml(String(e)) + '</div>';
    return;
  }
  if (services.length === 0) {
    container.innerHTML = '<div class="empty">还没有服务配置<br>添加一个即可开始使用</div>';
    // 首次使用：直接展示填写服务地址的表单
    openSheet(null);
    return;
  }
  container.innerHTML = services.map(s => `
    <button class="card" data-conn="${escapeHtml(s.name)}">
      <div class="card-top">
        <span class="card-name">${escapeHtml(s.name)}</span>
        <span class="card-menu" data-menu="${escapeHtml(s.name)}">⋯</span>
      </div>
      <span class="card-url">${escapeHtml(s.url)}</span>
      <div class="card-go"><span class="c">连接 →</span></div>
    </button>
  `).join('');
}

/* ---------- 表单弹层 ---------- */
async function openSheet(editName) {
  editingName = editName;
  document.getElementById('sheet-title').textContent = editName ? '编辑服务' : '添加服务';
  document.getElementById('sheet-save').textContent = editName ? '保存' : '保存并使用';
  if (editName) {
    try {
      const list = await invoke('list_services');
      const s = list.find(x => x.name === editName);
      if (s) {
        document.getElementById('f-name').value = s.name;
        document.getElementById('f-url').value = s.url;
        document.getElementById('f-token').value = s.token;
      }
    } catch (e) { /* 保持空表单 */ }
  } else {
    document.getElementById('f-name').value = '';
    document.getElementById('f-url').value = '';
    document.getElementById('f-token').value = '';
  }
  document.getElementById('sheet').classList.remove('hidden');
  setTimeout(() => document.getElementById('f-name').focus(), 120);
}

function hideSheet() {
  document.getElementById('sheet').classList.add('hidden');
  editingName = null;
}

/* ---------- ⋯ 菜单弹层 ---------- */
function openMenu(name) {
  menuTarget = name;
  document.getElementById('menu-name').textContent = name;
  document.getElementById('menu').classList.remove('hidden');
}
function hideMenu() {
  document.getElementById('menu').classList.add('hidden');
  menuTarget = null;
}

/* ---------- 事件 ---------- */
document.getElementById('service-list').addEventListener('click', e => {
  const menuBtn = e.target.closest('[data-menu]');
  if (menuBtn) {
    e.stopPropagation();
    openMenu(menuBtn.dataset.menu);
    return;
  }
  const card = e.target.closest('[data-conn]');
  if (card) invoke('open_service', { name: card.dataset.conn }).catch(err => toast('打开失败：' + err));
});

document.getElementById('add-btn').addEventListener('click', () => openSheet(null));
document.getElementById('sheet-cancel').addEventListener('click', hideSheet);
document.getElementById('sheet').addEventListener('click', e => { if (e.target.id === 'sheet') hideSheet(); });

document.getElementById('menu-edit').addEventListener('click', () => {
  const name = menuTarget;
  hideMenu();
  openSheet(name);
});
document.getElementById('menu-del').addEventListener('click', async () => {
  const name = menuTarget;
  hideMenu();
  if (!name) return;
  if (!confirm('确定删除 "' + name + '"？')) return;
  try {
    await invoke('delete_service', { name });
    toast('已删除');
    await loadServices();
  } catch (e) {
    toast('删除失败：' + e);
  }
});
document.getElementById('menu-cancel').addEventListener('click', hideMenu);
document.getElementById('menu').addEventListener('click', e => { if (e.target.id === 'menu') hideMenu(); });

document.getElementById('sheet-save').addEventListener('click', async () => {
  const name = document.getElementById('f-name').value.trim();
  const url = document.getElementById('f-url').value.trim();
  const token = document.getElementById('f-token').value.trim();
  if (!name) { toast('请填写名称'); return; }
  if (!url) { toast('请填写连接地址'); return; }
  if (!/^https?:\/\//i.test(url)) { toast('地址需以 http:// 或 https:// 开头'); return; }
  try {
    await invoke('save_service', { config: { name, url, token } });
    toast(editingName ? '已更新' : '已添加');
    hideSheet();
    await loadServices();
  } catch (e) {
    toast('保存失败：' + e);
  }
});

loadServices();
