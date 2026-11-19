// 迷你播放器（始终置顶小卡片）。主窗口通过 mini:state / mini:progress
// 事件推送播放状态，这里只负责展示与把控制命令转发给主窗口。
(function () {
    'use strict';

    const tauri = window.__TAURI__;
    if (!tauri) return;

    const { invoke } = tauri.core;
    const { listen, emitTo } = tauri.event;

    const titleEl = document.getElementById('mini-title');
    const artistEl = document.getElementById('mini-artist');
    const coverImg = document.getElementById('mini-cover-img');
    const coverPh = document.getElementById('mini-cover-ph');
    const toggleBtn = document.getElementById('mini-toggle');
    const prevBtn = document.getElementById('mini-prev');
    const nextBtn = document.getElementById('mini-next');
    const closeBtn = document.getElementById('mini-close');
    const progressBar = document.getElementById('mini-progress');
    const currentEl = document.getElementById('mini-current');
    const totalEl = document.getElementById('mini-total');

    let duration = 0;
    let dragging = false;

    function formatTime(s) {
        if (!s || isNaN(s) || !isFinite(s) || s < 0) return '0:00';
        s = Math.floor(s);
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        const sec = s % 60;
        if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
        return `${m}:${sec.toString().padStart(2, '0')}`;
    }

    function setState(s) {
        if (!s) return;
        // 无条件覆盖，避免清空列表后迷你卡残留上一首的标题/封面
        titleEl.textContent = s.title || '';
        artistEl.textContent = s.artist || '';
        if (s.cover) {
            coverImg.src = s.cover;
            coverImg.style.display = 'block';
            coverPh.style.display = 'none';
        } else {
            coverImg.removeAttribute('src');
            coverImg.style.display = 'none';
            coverPh.style.display = 'block';
        }
        toggleBtn.textContent = s.playing ? '⏸' : '▶';
        duration = s.duration || duration || 0;
        if (duration > 0) totalEl.textContent = formatTime(duration);
    }

    function setProgress(p) {
        if (!p || dragging) return;
        duration = p.duration || duration || 0;
        const d = duration;
        if (!d || d <= 0) return;
        currentEl.textContent = formatTime(p.current);
        totalEl.textContent = formatTime(d);
        progressBar.value = Math.min((p.current / d) * 1000, 1000);
    }

    listen('mini:state', (e) => setState(e.payload));
    listen('mini:progress', (e) => setProgress(e.payload));

    // 卡片页面加载完成后，通知主窗口“已就绪”。主窗口首次打开卡片时推的
    // mini:state/mini:progress 很可能早于这里的监听注册而丢失，导致刚弹出时
    // 卡片信息为空；收到 mini:ready 后主窗口会重新推送当前播放状态。
    emitTo('main', 'mini:ready').catch(() => {});

    toggleBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        invoke('mini_control', { action: 'toggle' });
    });
    prevBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        invoke('mini_control', { action: 'prev' });
    });
    nextBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        invoke('mini_control', { action: 'next' });
    });
    closeBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        invoke('close_mini');
    });

    // 与主窗口一致：input 只预览并暂存最新值，pointerup/change 才提交 seek。
    // 逐 input 事件直接 seek 会产生 IPC 风暴并反复打断播放。
    let pendingSeekPercent = null;
    progressBar.addEventListener('pointerdown', () => { dragging = true; });
    progressBar.addEventListener('pointerup', () => {
        if (pendingSeekPercent != null) commitSeek(); else dragging = false;
    });
    progressBar.addEventListener('pointercancel', () => {
        dragging = false;
        pendingSeekPercent = null;
    });
    progressBar.addEventListener('change', commitSeek);
    progressBar.addEventListener('input', () => {
        if (!duration || duration <= 0) return;
        const p = parseFloat(progressBar.value) / 1000;
        pendingSeekPercent = p;
        currentEl.textContent = formatTime(p * duration);
    });

    function commitSeek() {
        dragging = false;
        const percent = (pendingSeekPercent != null)
            ? pendingSeekPercent
            : (parseFloat(progressBar.value) / 1000);
        pendingSeekPercent = null;
        if (!duration || duration <= 0 || !isFinite(percent)) return;
        const target = percent * duration;
        currentEl.textContent = formatTime(target);
        invoke('mini_control', { action: 'seek', value: target });
    }

    // 点击卡片空白处不再弹回主窗口（卡片整体是可拖动区域，
    // 恢复主窗口请用右上角的 ✕ 按钮：它会关闭卡片并取消最小化主窗口）。
})();
