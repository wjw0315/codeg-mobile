// WebView 无打包器，走 withGlobalTauri 注入的全局 API
const invoke = window.__TAURI__.core.invoke;

let editingName = null;
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
  try {
    const services = await invoke('list_services');
    if (services.length === 0) {
      container.innerHTML = '<div class="empty">暂无服务，先添加一个服务配置</div>';
      // 首次使用：直接展示填写服务地址的表单
      document.getElementById('modal-title').textContent = '添加服务';
      showModal();
      return;
    }
    container.innerHTML = services.map(s => `
      <div class="card">
        <div class="card-info">
          <div class="card-name">${escapeHtml(s.name)}</div>
          <div class="card-url">${escapeHtml(s.url)}</div>
        </div>
        <div class="card-actions">
          <button class="btn btn-primary" onclick="openService('${escapeHtml(s.name)}')">连接</button>
          <button class="btn btn-edit" onclick="editService('${escapeHtml(s.name)}')">编辑</button>
          <button class="btn btn-del" onclick="deleteService('${escapeHtml(s.name)}')">删除</button>
        </div>
      </div>
    `).join('');
  } catch (e) {
    container.innerHTML = '<div class="empty">加载失败：' + escapeHtml(String(e)) + '</div>';
  }
}

window.openService = async function (name) {
  try {
    await invoke('open_service', { name });
  } catch (e) {
    toast('打开失败：' + e);
  }
};

window.editService = function (name) {
  editingName = name;
  document.getElementById('modal-title').textContent = '编辑服务';
  const services = document.querySelectorAll('.card');
  // 从当前列表找到该服务填充表单
  invoke('list_services').then(list => {
    const s = list.find(x => x.name === name);
    if (s) {
      document.getElementById('f-name').value = s.name;
      document.getElementById('f-url').value = s.url;
      document.getElementById('f-token').value = s.token;
    }
  });
  showModal();
};

window.deleteService = async function (name) {
  if (!confirm('确定删除 "' + name + '"？')) return;
  try {
    await invoke('delete_service', { name });
    toast('已删除');
    await loadServices();
  } catch (e) {
    toast('删除失败：' + e);
  }
};

function showModal() {
  document.getElementById('modal').classList.remove('hidden');
  setTimeout(() => document.getElementById('f-name').focus(), 100);
}

function hideModal() {
  document.getElementById('modal').classList.add('hidden');
  editingName = null;
  document.getElementById('f-name').value = '';
  document.getElementById('f-url').value = '';
  document.getElementById('f-token').value = '';
}

document.getElementById('add-btn').addEventListener('click', () => {
  editingName = null;
  document.getElementById('modal-title').textContent = '添加服务';
  hideModal();
  showModal();
  setTimeout(() => document.getElementById('f-name').focus(), 100);
});

document.getElementById('modal-cancel').addEventListener('click', hideModal);

document.getElementById('modal').addEventListener('click', (e) => {
  if (e.target.id === 'modal') hideModal();
});

document.getElementById('modal-save').addEventListener('click', async () => {
  const name = document.getElementById('f-name').value.trim();
  const url = document.getElementById('f-url').value.trim();
  const token = document.getElementById('f-token').value.trim();
  if (!name) { toast('请填写名称'); return; }
  if (!url) { toast('请填写连接地址'); return; }
  if (!/^https?:\/\//i.test(url)) { toast('地址需以 http:// 或 https:// 开头'); return; }
  try {
    await invoke('save_service', {
      config: { name, url, token }
    });
    toast(editingName ? '已更新' : '已添加');
    hideModal();
    await loadServices();
  } catch (e) {
    toast('保存失败：' + e);
  }
});

loadServices();
