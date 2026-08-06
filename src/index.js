console.log('[Renderer] 渲染进程脚本开始执行');

document.addEventListener('DOMContentLoaded', () => {
    console.log('[Renderer] DOM 已加载');

    // ---- 元素引用 ----
    const audioPlayer = document.getElementById('audio-player');
    const videoPlayer = document.getElementById('video-player');
    const videoWrap = document.getElementById('video-wrap');
    const coverSection = document.getElementById('cover-section');
    const fileInfo = document.getElementById('file-info');
    const statusEl = document.getElementById('status');
    const openBtn = document.getElementById('open-btn');
    const openFolderBtn = document.getElementById('open-folder-btn');
    const playBtn = document.getElementById('play-btn');
    const pauseBtn = document.getElementById('pause-btn');
    const menuToggleBtn = document.getElementById('menu-toggle-btn');
    const closePlaylistBtn = document.getElementById('close-playlist-btn');
    const playlistPanel = document.getElementById('playlist-panel');

    const playToggleBtn = document.getElementById('play-toggle-btn');
    const prevBtn = document.getElementById('prev-btn');
    const nextBtn = document.getElementById('next-btn');
    const progressBar = document.getElementById('progress-bar');
    const volumeSlider = document.getElementById('volume-slider');
    const volumeIcon = document.getElementById('volume-icon');
    const currentTimeEl = document.getElementById('current-time');
    const totalTimeEl = document.getElementById('total-time');
    const playlistEl = document.getElementById('playlist');
    const playlistCount = document.getElementById('playlist-count');
    const playlistEmpty = document.getElementById('playlist-empty');
    const clearPlaylistBtn = document.getElementById('clear-playlist-btn');
    const songTitle = document.getElementById('song-title');
    const songArtist = document.getElementById('song-artist');
    const coverImage = document.getElementById('cover-image');
    const coverPlaceholder = document.getElementById('cover-placeholder');
    const coverArt = document.getElementById('cover-art');

    // ---- 顶部栏按钮 ----
    const minBtn = document.getElementById('min-btn');
    const closeBtn = document.getElementById('close-btn');
    const settingsBtn = document.getElementById('settings-btn');
    const pinBtn = document.getElementById('pin-btn');
    const winFullBtn = document.getElementById('window-fullscreen-btn');
    const contentFullBtn = document.getElementById('content-fullscreen-btn');
    const miniBtn = document.getElementById('mini-btn');

    // ---- 视频浮层控件 ----
    const videoPlayBtn = document.getElementById('video-play');
    const videoFullscreenBtn = document.getElementById('video-fullscreen');
    const videoProgressBar = document.getElementById('video-progress');
    const videoCurrentEl = document.getElementById('video-current');
    const videoTotalEl = document.getElementById('video-total');
    const seekTooltip = document.getElementById('seek-tooltip');

    // ---- Tauri API ----
    const tauri = window.__TAURI__;
    if (!tauri) {
        console.error('[Renderer] ❌ Tauri API 未定义！');
        if (statusEl) statusEl.textContent = '❌ API 加载失败';
        return;
    }
    const { invoke, convertFileSrc } = tauri.core;
    const { getCurrentWindow } = tauri.window;
    const { listen, emitTo } = tauri.event;
    const mainWindow = getCurrentWindow();
    console.log('[Renderer] ✅ Tauri API 可用');

    // 把诊断信息同步写到 Rust 日志文件（<cache>/player.log），供冻结问题复盘。
    function sendLog(msg) {
        invoke('log_app', { level: 'JS', msg: String(msg) }).catch(() => {});
    }
    // 捕获未处理异常 / 全局错误，防止“无任何报错”的情况。
    window.addEventListener('error', (e) => {
        sendLog('window.onerror: ' + e.message + ' @ ' + e.filename + ':' + e.lineno);
    });
    window.addEventListener('unhandledrejection', (e) => {
        sendLog('unhandledrejection: ' + (e.reason && e.reason.message ? e.reason.message : e.reason));
    });

    // ============================================================
    // 2. 播放列表数据
    // ============================================================
    let playlist = [];
    // let fixedDuration = 0;     // 固定总时长（秒），用于显示和进度计算
    let currentIndex = -1;
    let isPlaying = false;
    let currentCoverUrl = null;

    // 播放模式：0 列表循环 / 1 单曲循环 / 2 随机播放
    let playMode = 0;
    const modeIcons = ['🔁', '🔂', '🔀'];
    const modeTitles = ['列表循环', '单曲循环', '随机播放'];

    // 曲目切换时的“待自动播放”标记：等 canplay 元数据就绪后再 play，避免
    // WebKitGTK 把提前 play 的音频当直播流（时长跟随缓冲位置增长）。
    let pendingAutoplay = false;
    // 当前等待 canplay 的自动播放监听器（同一元素只保留一个，防止跨曲目重复触发）
    let autoplayCb = null;

    // 当前正在播放的媒体元素（audio 或 video）
    function activeMedia() {
        const item = playlist[currentIndex];
        return item && item.has_video ? videoPlayer : audioPlayer;
    }

    function isFiniteDuration(d) {
        return typeof d === 'number' && isFinite(d) && !isNaN(d) && d > 0;
    }

    // 媒体元素自己上报的时长才是真实可播放/可 seek 的范围；浏览器给不出
    // （Infinity/NaN/0，常见于 HTTP 视频流刚加载时）才回退到 ffprobe 元数据
    // 时长，这样进度条始终能映射、始终能 seek，且不会用错误时长把 seek 抬到
    // 流末尾之外触发 ended 自动切歌。
    function currentDuration() {
        const media = activeMedia();
        if (media && isFiniteDuration(media.duration)) {
            return media.duration;
        }
        const item = playlist[currentIndex];
        if (item && isFiniteDuration(item.duration)) {
            return item.duration;
        }
        return 0;
    }

    // ============================================================
    // 3. 核心函数
    // ============================================================
    function formatTime(seconds) {
        if (!isFiniteDuration(seconds)) return '0:00';
        const m = Math.floor(seconds / 60);
        const s = Math.floor(seconds % 60);
        return `${m}:${s.toString().padStart(2, '0')}`;
    }

    function updatePlaylistUI() {
        if (!playlistEl) return;
        playlistEl.innerHTML = '';
        if (playlist.length === 0) {
            playlistEmpty.style.display = 'flex';
            playlistEl.style.display = 'none';
            playlistCount.textContent = '(0 首)';
            return;
        }
        playlistEmpty.style.display = 'none';
        playlistEl.style.display = 'block';
        playlistCount.textContent = `(${playlist.length} 首)`;

        playlist.forEach((item, idx) => {
            const li = document.createElement('li');
            li.className = 'playlist-item' + (idx === currentIndex ? ' active' : '');
            li.dataset.index = idx;

            const indexSpan = document.createElement('span');
            indexSpan.className = 'item-index';
            indexSpan.textContent = (idx + 1).toString().padStart(2, '0');

            const nameSpan = document.createElement('span');
            nameSpan.className = 'item-name';
            nameSpan.textContent = (item.has_video ? '🎬 ' : '') + (item.title || item.name || '未知歌曲');

            const durSpan = document.createElement('span');
            durSpan.className = 'item-duration';
            durSpan.textContent = item.duration ? formatTime(item.duration) : '--:--';

            const removeBtn = document.createElement('button');
            removeBtn.className = 'item-remove';
            removeBtn.textContent = '✕';
            removeBtn.addEventListener('click', (e) => {
                e.stopPropagation();
                removeFromPlaylist(idx);
            });

            li.appendChild(indexSpan);
            li.appendChild(nameSpan);
            li.appendChild(durSpan);
            li.appendChild(removeBtn);

            li.addEventListener('click', () => {
                playIndex(idx);
                closePlaylist();
            });

            playlistEl.appendChild(li);
        });
    }

    function addSingleFile(metadata) {
        if (!metadata) return false;
        if (playlist.some(item => item.path === metadata.path)) {
            setStatus('⏳ 文件已在列表中', 'info');
            return false;
        }
        playlist.push({
            path: metadata.path,
            name: metadata.name,
            duration: metadata.duration || 0,
            title: metadata.title || metadata.name,
            artist: metadata.artist || '未知艺术家',
            has_video: !!metadata.has_video,
        });
        updatePlaylistUI();
        if (currentIndex === -1) {
            playIndex(playlist.length - 1);
        }
        return true;
    }

    function addMultipleFiles(metadataList) {
        if (!metadataList || metadataList.length === 0) {
            setStatus('⚠️ 文件夹中没有支持的音频文件', 'error');
            return;
        }
        let added = 0;
        metadataList.forEach(meta => {
            if (!playlist.some(item => item.path === meta.path)) {
                playlist.push({
                    path: meta.path,
                    name: meta.name,
                    duration: meta.duration || 0,
                    title: meta.title || meta.name,
                    artist: meta.artist || '未知艺术家',
                    has_video: !!meta.has_video,
                });
                added++;
            }
        });
        updatePlaylistUI();
        if (added > 0 && currentIndex === -1) {
            playIndex(0);
        }
        setStatus(`✅ 成功导入 ${added} 首歌曲`, 'success');
    }

    function removeFromPlaylist(index) {
        if (index < 0 || index >= playlist.length) return;
        const wasPlaying = (index === currentIndex && isPlaying);
        playlist.splice(index, 1);
        if (currentIndex === index) {
            currentIndex = -1;
            if (playlist.length > 0) {
                const newIdx = Math.min(index, playlist.length - 1);
                playIndex(newIdx);
            } else {
                stopPlayback();
                setStatus('⏹ 列表已清空', 'info');
            }
        } else if (currentIndex > index) {
            currentIndex--;
        }
        updatePlaylistUI();
    }

    function clearPlaylist() {
        if (playlist.length === 0) return;
        stopPlayback();
        currentIndex = -1;
        playlist = [];
        updatePlaylistUI();
        updateSongInfo(null);
        updatePlayToggleIcon();
        updateCover(null);
        setStatus('🗑 列表已清空', 'info');
    }

    function stopPlayback() {
        // 使任何进行中的自动播放失效，防止清空列表/停止后旧曲目又播起来。
        pendingAutoplay = false;
        if (autoplayCb) { audioPlayer.removeEventListener('canplay', autoplayCb); videoPlayer.removeEventListener('canplay', autoplayCb); autoplayCb = null; }
        try { audioPlayer.pause(); audioPlayer.removeAttribute('src'); audioPlayer.load(); } catch (e) {}
        try { videoPlayer.pause(); videoPlayer.removeAttribute('src'); videoPlayer.load(); } catch (e) {}
        isPlaying = false;
        updatePlayToggleIcon();
        videoWrap.classList.remove('show');
        if (coverSection) coverSection.style.display = '';
        emitMiniState();
    }

    async function playIndex(index) {
        if (index < 0 || index >= playlist.length) return;
        currentIndex = index;
        const item = playlist[index];
        if (!item) return;
        const isVideo = !!item.has_video;

        setStatus(isVideo ? '⏳ 准备视频...' : '⏳ 解码中...', 'info');

        // 先停止另一个媒体元素
        const other = isVideo ? audioPlayer : videoPlayer;
        try { other.pause(); other.removeAttribute('src'); other.load(); } catch (e) {}

        try {
            const url = isVideo
                ? await invoke('video_url', { path: item.path, mode: 'http' })
                : await invoke('stream_url', { path: item.path });

            const media = isVideo ? videoPlayer : audioPlayer;
            media.src = url;
            media.load();

            // ⚠️ 关键：不要立刻 play()。WebKitGTK 若在元数据就绪前就 play，
            // 会把音频当成“直播流”：duration 恒等于已缓冲到的位置、seekable
            // 跟随播放头增长，于是总时长显示成当前播放时间、进度条无法 seek/click。
            // 等 canplay（元数据就绪）后再自动播放，时长与 seekable 才会按文件
            // 真实时长建立，进度条才能正常映射与拖动。
            pendingAutoplay = true;
            const autoplayOnce = function () {
                media.removeEventListener('canplay', autoplayOnce);
                if (autoplayCb === autoplayOnce) autoplayCb = null;
                if (pendingAutoplay) {
                    pendingAutoplay = false;
                    media.play().catch(err => console.warn('[Renderer] 自动播放被阻止:', err));
                }
            };
            // 移除上一首遗留的 canplay 监听，避免跨曲目重复触发 play()
            if (autoplayCb) media.removeEventListener('canplay', autoplayCb);
            autoplayCb = autoplayOnce;
            media.addEventListener('canplay', autoplayCb);

            // 视频：显示视频区填满主区域，隐藏封面；音频反之
            if (isVideo) {
                videoWrap.classList.add('show');
                if (coverSection) coverSection.style.display = 'none';
                updateCover(null);
                if (videoProgressBar) videoProgressBar.value = 0;
                if (videoCurrentEl) videoCurrentEl.textContent = '0:00';
                if (videoTotalEl) videoTotalEl.textContent = '0:00';
                videoWrap.classList.remove('idle');
                videoControlsWake();
            } else {
                videoWrap.classList.remove('show');
                if (coverSection) coverSection.style.display = '';
            }

            // 自动播放已委托给 canplay（见上方 pendingAutoplay），不在这里 play()，
            // 以避免 WebKitGTK 把提前 play 的音频当成直播流。
            isPlaying = true;
            updatePlayToggleIcon();
            updateSongInfo(item);
            // 封面按需加载：只在播放这一首时获取，避免一次性导入所有封面
            if (!isVideo) {
                loadCover(item.path);
            }
            updatePlaylistUI();
            emitMiniState();
            setStatus(`▶ 正在播放: ${item.title}`, 'success');
        } catch (err) {
            console.error('[Renderer] 解码失败:', err);
            setStatus('❌ 解码失败: ' + (err && err.message ? err.message : err), 'error');
            // 尝试自动跳到下一首
            if (playlist.length > 0) {
                setTimeout(() => playNext(), 600);
            }
        }
    }

    function playNext() {
        if (playlist.length === 0) return;
        let idx;
        if (playMode === 2) {
            do { idx = Math.floor(Math.random() * playlist.length); } while (idx === currentIndex && playlist.length > 1);
        } else {
            idx = (currentIndex + 1) % playlist.length;
        }
        playIndex(idx);
    }

    function playPrev() {
        if (playlist.length === 0) return;
        let idx;
        if (playMode === 2) {
            do { idx = Math.floor(Math.random() * playlist.length); } while (idx === currentIndex && playlist.length > 1);
        } else {
            idx = (currentIndex - 1 + playlist.length) % playlist.length;
        }
        playIndex(idx);
    }

    function togglePlayPause() {
        if (playlist.length === 0) {
            setStatus('⚠️ 请先添加音乐文件', 'error');
            return;
        }
        if (currentIndex === -1) {
            playIndex(0);
            return;
        }
        const media = activeMedia();
        if (isPlaying) {
            media.pause();
            pendingAutoplay = false;
            isPlaying = false;
            updatePlayToggleIcon();
            setStatus('⏸ 已暂停', 'info');
        } else {
            media.play().catch(err => {
                console.warn('[Renderer] 播放失败:', err);
                setStatus('❌ 播放失败: ' + err.message, 'error');
            });
            isPlaying = true;
            updatePlayToggleIcon();
            const item = playlist[currentIndex];
            setStatus(`▶ 播放中: ${item?.title || ''}`, 'success');
        }
        emitMiniState();
    }

    function updateSongInfo(item) {
        const name = item?.title || '未选择歌曲';
        const artist = item?.artist || '本地文件';
        songTitle.textContent = name;
        songArtist.textContent = artist;
        if (!item) {
            currentTimeEl.textContent = '0:00';
            totalTimeEl.textContent = '0:00';
            progressBar.value = 0;
        }
        // 控制文字滚动
        const titleEl = document.querySelector('.song-name');
        if (titleEl) {
            let inner = titleEl.querySelector('.song-name-inner');
            if (!inner) {
                const span = document.createElement('span');
                span.className = 'song-name-inner';
                span.textContent = name;
                titleEl.textContent = '';
                titleEl.appendChild(span);
                inner = span;
            } else {
                inner.textContent = name;
            }
            const isOverflow = inner.scrollWidth > titleEl.clientWidth;
            titleEl.classList.toggle('long', isOverflow);
            if (isOverflow) {
                inner.style.animation = 'none';
                void inner.offsetHeight;
                inner.style.animation = 'scrollText 12s linear infinite';
            } else {
                inner.style.animation = 'none';
            }
        }
    }

    // 按需加载当前播放音频的封面（一次只加载一首）
    async function loadCover(path) {
        try {
            const coverUrl = await invoke('get_cover', { path: path });
            updateCover(coverUrl || null);
        } catch (e) {
            console.warn('[Renderer] 获取封面失败:', e);
            updateCover(null);
        }
    }

    function updateCover(coverData) {
        currentCoverUrl = coverData || null;
        coverImage.onerror = null;
        if (currentCoverUrl) {
            coverImage.src = currentCoverUrl;
            coverImage.style.display = 'block';
            coverPlaceholder.style.display = 'none';
            coverImage.onerror = function() {
                console.warn('[Renderer] 封面图片加载失败，回退到默认图标');
                coverImage.style.display = 'none';
                coverPlaceholder.style.display = 'block';
                coverImage.onerror = null;
            };
        } else {
            coverImage.removeAttribute('src');
            coverImage.style.display = 'none';
            coverPlaceholder.style.display = 'block';
        }
        emitMiniState();
    }

    function updatePlayToggleIcon() {
        if (playToggleBtn) {
            playToggleBtn.textContent = isPlaying ? '⏸' : '▶';
        }
    }

    function setStatus(msg, type = 'info') {
        if (!statusEl) return;
        statusEl.textContent = msg;
        statusEl.className = 'status-bar ' + type;
    }

    // 静音前的音量：点击喇叭静音/恢复时使用（也随每次音量调整更新）
    let lastVolume = 40;

    function updateVolumeUI(val) {
        const v = parseFloat(val);
        if (isNaN(v)) return;
        if (v > 0) lastVolume = v;
        audioPlayer.volume = v / 100;
        videoPlayer.volume = v / 100;
        if (v === 0) volumeIcon.textContent = '🔇';
        else if (v < 30) volumeIcon.textContent = '🔈';
        else if (v < 70) volumeIcon.textContent = '🔉';
        else volumeIcon.textContent = '🔊';
    }

    // ============================================================
    // 4. 迷你播放器同步（置顶小卡片）
    // ============================================================
    function emitMiniState() {
        if (!emitTo) return;
        const item = playlist[currentIndex];
        emitTo('mini', 'mini:state', {
            title: item?.title || '',
            artist: item?.artist || '',
            cover: currentCoverUrl || null,
            playing: !!isPlaying,
            has_video: !!(item && item.has_video),
            duration: currentDuration(),
        }).catch(() => {});
    }

    function emitMiniProgress() {
        if (!emitTo) return;
        const media = activeMedia();
        if (!media) return;
        emitTo('mini', 'mini:progress', {
            current: media.currentTime || 0,
            duration: currentDuration(),
        }).catch(() => {});
    }

    function openPlaylist() {
        playlistPanel.classList.add('open');
    }
    function closePlaylist() {
        playlistPanel.classList.remove('open');
    }
    function togglePlaylist() {
        playlistPanel.classList.toggle('open');
    }

    // ============================================================
    // 5. 主题系统（按月变化的主题 + 自定义主题，见 theme.js）
    // ============================================================
    async function loadTheme() {
        const theme = await invoke('store_get', { key: 'theme' }) || 'auto';
        await Theme.applyTheme(theme);
    }

    // ============================================================
    // 6. 事件绑定
    // ============================================================

    // ---- 窗口控制 ----
    if (minBtn) {
        minBtn.addEventListener('click', () => {
            mainWindow.minimize();
        });
    }
    if (closeBtn) {
        closeBtn.addEventListener('click', () => {
            mainWindow.close();
        });
    }

    // ---- 置顶 ----
    let isOnTop = false;
    if (pinBtn) {
        pinBtn.addEventListener('click', async () => {
            isOnTop = !isOnTop;
            try {
                await mainWindow.setAlwaysOnTop(isOnTop);
                pinBtn.classList.toggle('active', isOnTop);
                pinBtn.title = isOnTop ? '取消置顶' : '置顶';
            } catch (e) {
                console.warn('[Renderer] 置顶切换失败:', e);
            }
        });
    }

    // ---- 窗口全屏 / 内容全屏 ----
    let inContentFullscreen = false;
    // 进入内容全屏之前，窗口是否已经处于（窗口）全屏状态；退出内容全屏时
    // 要恢复到这个状态，而不是无条件回到非全屏或停在窗口全屏。
    let wasFullscreenBeforeContent = false;

    async function syncFullscreenButtons() {
        try {
            const full = await mainWindow.isFullscreen();
            if (winFullBtn) winFullBtn.classList.toggle('active', full && !inContentFullscreen);
            if (contentFullBtn) contentFullBtn.classList.toggle('active', inContentFullscreen);
        } catch (e) {}
    }

    async function enterContentFullscreen() {
        wasFullscreenBeforeContent = await mainWindow.isFullscreen().catch(() => false);
        if (!wasFullscreenBeforeContent) {
            await mainWindow.setFullscreen(true);
        }
        inContentFullscreen = true;
        document.body.classList.add('content-fullscreen');
        await syncFullscreenButtons();
    }

    async function exitContentFullscreen() {
        inContentFullscreen = false;
        document.body.classList.remove('content-fullscreen');
        // 回到进入内容全屏前的窗口状态（若此前非全屏，则恢复非全屏）
        await mainWindow.setFullscreen(wasFullscreenBeforeContent);
        await syncFullscreenButtons();
    }

    async function toggleContentFullscreen() {
        try {
            if (inContentFullscreen) {
                await exitContentFullscreen();
            } else {
                await enterContentFullscreen();
            }
        } catch (e) {
            console.warn('[Renderer] 内容全屏切换失败:', e);
        }
    }

    if (winFullBtn) {
        winFullBtn.addEventListener('click', async () => {
            try {
                if (inContentFullscreen) await exitContentFullscreen();
                const full = await mainWindow.isFullscreen();
                await mainWindow.setFullscreen(!full);
                await syncFullscreenButtons();
            } catch (e) {
                console.warn('[Renderer] 窗口全屏切换失败:', e);
            }
        });
    }

    if (contentFullBtn) {
        contentFullBtn.addEventListener('click', toggleContentFullscreen);
    }

    // ---- 内容全屏的悬浮退出按钮 ----
    const exitContentFullscreenBtn = document.getElementById('exit-content-fullscreen-btn');
    if (exitContentFullscreenBtn) {
        exitContentFullscreenBtn.addEventListener('click', () => {
            exitContentFullscreen().catch(() => {});
        });
    }

    // ---- 迷你播放器按钮 ----
    if (miniBtn) {
        miniBtn.addEventListener('click', async () => {
            try {
                await invoke('open_mini');
                // 打开卡片后立即推送当前播放状态，让封面/标题/按钮马上更新
                emitMiniState();
                emitMiniProgress();
            } catch (e) {
                setStatus('❌ 无法打开迷你播放器', 'error');
            }
        });
    }

    // ---- 设置按钮（点击一次打开，再次点击或点设置页叉号关闭） ----
    if (settingsBtn) {
        settingsBtn.addEventListener('click', async () => {
            try {
                const opened = await invoke('open_settings');
                settingsBtn.classList.toggle('active-settings', !!opened);
            } catch (error) {
                console.error('[Renderer] 创建设置窗口失败:', error);
                setStatus('❌ 无法打开设置窗口', 'error');
            }
        });
    }

    // ---- 视频浮层：播放/暂停 + 内容全屏 ----
    if (videoPlayBtn) {
        videoPlayBtn.addEventListener('click', () => {
            if (videoPlayer.paused) {
                videoPlayer.play().catch(() => {});
            } else {
                videoPlayer.pause();
            }
        });
    }
    if (videoFullscreenBtn) {
        videoFullscreenBtn.addEventListener('click', () => {
            toggleContentFullscreen();
        });
    }
    videoWrap.addEventListener('dblclick', () => {
        toggleContentFullscreen();
    });
    // 单击视频切换播放/暂停（video 本身 pointer-events:none，事件落在容器上；
    // 点击浮层按钮/进度条时忽略）
    videoWrap.addEventListener('click', (e) => {
        if (e.target.closest('button') || e.target.closest('input')) return;
        if (videoPlayer.paused) {
            videoPlayer.play().catch(() => {});
        } else {
            videoPlayer.pause();
        }
    });

    // ---- 视频控制浮层自动隐藏：播放中鼠标不动约 2.5s 后隐藏视频上的控制层
    //      （进度条/播放暂停等），只留主窗口底部进度条；鼠标一动即恢复 ----
    let videoControlsTimer = null;
    function videoControlsWake() {
        if (!videoWrap.classList.contains('show')) return;
        videoWrap.classList.remove('idle');
        if (videoControlsTimer) clearTimeout(videoControlsTimer);
        videoControlsTimer = setTimeout(() => {
            if (!videoPlayer.paused) videoWrap.classList.add('idle');
        }, 2500);
    }
    videoWrap.addEventListener('pointermove', videoControlsWake);
    videoWrap.addEventListener('pointerdown', videoControlsWake);
    videoWrap.addEventListener('pointerenter', videoControlsWake);
    videoWrap.addEventListener('pointerleave', () => {
        if (videoControlsTimer) clearTimeout(videoControlsTimer);
        if (!videoPlayer.paused) videoWrap.classList.add('idle');
    });

    // ---- 点击列表外部关闭播放列表 ----
    document.addEventListener('click', (e) => {
        if (!playlistPanel.classList.contains('open')) return;
        if (playlistPanel.contains(e.target)) return;
        if (menuToggleBtn && menuToggleBtn.contains(e.target)) return;
        closePlaylist();
    });

    // ---- 主题变化 / 设置关闭 / 迷你播放器命令监听 ----
    if (tauri.event) {
        listen('theme:changed', (e) => {
            Theme.applyTheme(e.payload);
        });
        // 设置页通过叉号关闭时，恢复设置按钮高亮状态
        listen('settings:closed', () => {
            if (settingsBtn) settingsBtn.classList.remove('active-settings');
        });
        // 迷你播放器发来的控制指令
        listen('mini:command', (e) => {
            const p = e.payload || {};
            const action = p.action;
            if (action === 'toggle') {
                togglePlayPause();
            } else if (action === 'next') {
                playNext();
            } else if (action === 'prev') {
                playPrev();
            } else if (action === 'seek') {
                const v = parseFloat(p.value);
                if (isFinite(v) && v >= 0) {
                    const media = activeMedia();
                    if (media) media.currentTime = clampSeekTarget(media, v);
                }
            }
        });
        // 迷你卡片就绪后，重新推送一次当前状态与进度，保证刚打开时信息不缺失
        listen('mini:ready', () => {
            emitMiniState();
            emitMiniProgress();
        });
        // 备份恢复：点 ✕ 关闭卡片时，Rust 端会先尝试恢复主窗口；这里再兜底
        // 一次，确保最小化的主窗口一定被拉回前台（含被窗口管理器拦截的情况）。
        listen('mini:close-restore', () => {
            mainWindow.unminimize().catch(() => {});
            mainWindow.show().catch(() => {});
            mainWindow.setFocus().catch(() => {});
        });
    }

    // ---- 文件操作 ----
    if (openBtn) {
        openBtn.addEventListener('click', async () => {
            setStatus('⏳ 选择文件中...', 'info');
            try {
                const metadata = await invoke('open_file');
                if (metadata) {
                    const added = addSingleFile(metadata);
                    if (added) {
                        fileInfo.textContent = `📁 ${metadata.name}`;
                        setStatus(`✅ 已添加: ${metadata.title}`, 'success');
                    }
                } else {
                    setStatus('❌ 未选择任何文件', 'error');
                }
            } catch (error) {
                setStatus('❌ 错误: ' + error.message, 'error');
            }
        });
    }

    if (openFolderBtn) {
        openFolderBtn.addEventListener('click', async () => {
            setStatus('⏳ 扫描文件夹中...', 'info');
            try {
                const metadataList = await invoke('open_folder');
                addMultipleFiles(metadataList);
                if (metadataList.length > 0) {
                    fileInfo.textContent = `📂 导入 ${metadataList.length} 个文件`;
                } else {
                    setStatus('⚠️ 文件夹中没有支持的音频文件', 'error');
                }
            } catch (error) {
                setStatus('❌ 错误: ' + error.message, 'error');
            }
        });
    }

    // 播放/暂停
    if (playBtn) {
        playBtn.addEventListener('click', () => {
            const media = activeMedia();
            if (media && media.src) {
                media.play();
                isPlaying = true;
                updatePlayToggleIcon();
                const item = playlist[currentIndex];
                setStatus(`▶ 播放中: ${item?.title || ''}`, 'success');
                emitMiniState();
            } else {
                setStatus('⚠️ 请先选择音乐文件', 'error');
            }
        });
    }
    if (pauseBtn) {
        pauseBtn.addEventListener('click', () => {
            const media = activeMedia();
            if (media) {
                media.pause();
                isPlaying = false;
                updatePlayToggleIcon();
                setStatus('⏸ 已暂停', 'info');
                emitMiniState();
            }
        });
    }

    // 主播放切换
    if (playToggleBtn) {
        playToggleBtn.addEventListener('click', togglePlayPause);
    }

    // 上一首/下一首
    if (prevBtn) prevBtn.addEventListener('click', playPrev);
    if (nextBtn) nextBtn.addEventListener('click', playNext);

    // 播放模式按钮（列表循环 / 单曲循环 / 随机播放，不显示状态提示行）
    const modeBtn = document.getElementById('mode-btn');
    function updateModeBtn() {
        if (!modeBtn) return;
        modeBtn.textContent = modeIcons[playMode];
        modeBtn.title = modeTitles[playMode];
    }
    if (modeBtn) {
        modeBtn.addEventListener('click', () => {
            playMode = (playMode + 1) % 3;
            updateModeBtn();
        });
    }

    // 清空列表
    if (clearPlaylistBtn) {
        clearPlaylistBtn.addEventListener('click', clearPlaylist);
    }

    // 菜单按钮
    if (menuToggleBtn) {
        menuToggleBtn.addEventListener('click', togglePlaylist);
    }
    if (closePlaylistBtn) {
        closePlaylistBtn.addEventListener('click', closePlaylist);
    }

    // ---- 进度条（音频/视频共用，使用已知时长映射，始终可 seek） ----
    let draggingProgress = false;
    let draggingVideoProgress = false;
    // 记录 input 事件里滑块的最新位置。WebKitGTK 的 range 滑块在 pointerup
    // 触发时 value 可能尚未更新（常见是仍停在 0 或上一首结尾的 1000），此时
    // 直接读 value 会拿到旧值：轻则点击“看似没反应”（进度条失效），重则把
    // currentTime 设到结尾触发 ended、自动切到下一首。所以 seek 一律使用
    // input/change 里拿到的最新值，不再依赖 pointerup 那一刻的 value。
    let pendingProgressPercent = null;
    let pendingVideoPercent = null;

    // 计算一次 seek 的目标秒数，并收窄到“真实可播放范围”：
    // 1. ffprobe 时长可能比实际流略长，若直接 seek 到 ffprobe 末端会越界，
    //    浏览器立刻触发 ended，出现“点一下进度条就跳下一首”；
    // 2. 媒体自身已报出有限时长或 seekable 范围时以其为准；
    // 3. 留 0.05s 余量，避免恰好落在流末尾。
    function clampSeekTarget(media, target) {
        if (!media || !isFinite(target) || target <= 0) return 0;
        const dur = currentDuration();
        let upper = dur > 0 ? dur : 0;
        const mediaDur = media.duration;
        if (isFiniteDuration(mediaDur)) {
            upper = upper > 0 ? Math.min(upper, mediaDur) : mediaDur;
        } else if (media.seekable && media.seekable.length > 0) {
            const sEnd = media.seekable.end(media.seekable.length - 1);
            if (isFiniteDuration(sEnd)) {
                upper = upper > 0 ? Math.min(upper, sEnd) : sEnd;
            }
        }
        if (upper <= 0) return Math.max(target, 0);
        return Math.min(Math.max(target, 0), Math.max(0, upper - 0.05));
    }

    function seekTo(media, percent) {
        if (!media) return;
        const dur = currentDuration();
        if (!dur || dur <= 0) return;
        const target = clampSeekTarget(media, percent * dur);
        // 直接设置，并立即更新UI（防止UI滞后）
        media.currentTime = target;
        // 手动更新UI
        updateSeekUI(media, target, dur);
    }


    function updateSeekUI(media, current, duration) {
        const dur = duration || currentDuration();
        if (dur > 0) {
            const percent = Math.min((current / dur) * 1000, 1000);
            progressBar.value = percent;
            currentTimeEl.textContent = formatTime(current);
            totalTimeEl.textContent = formatTime(dur);
            if (media === videoPlayer && videoProgressBar) {
                videoProgressBar.value = percent;
                if (videoCurrentEl) videoCurrentEl.textContent = formatTime(current);
                if (videoTotalEl) videoTotalEl.textContent = formatTime(dur);
            }
            emitMiniProgress();
        }
    }

    // 执行一次真正的 seek：优先用 input 里暂存的最新值，避免 pointerup 旧值问题。
    function commitProgressSeek() {
        draggingProgress = false;
        const percent = (pendingProgressPercent != null)
            ? pendingProgressPercent
            : (parseFloat(progressBar.value) / 1000);
        pendingProgressPercent = null;
        const media = activeMedia();
        if (media && isFinite(percent)) {
            seekTo(media, percent);
        }
    }

    function commitVideoSeek() {
        draggingVideoProgress = false;
        const percent = (pendingVideoPercent != null)
            ? pendingVideoPercent
            : (parseFloat(videoProgressBar.value) / 1000);
        pendingVideoPercent = null;
        if (isFinite(percent)) {
            seekTo(videoPlayer, percent);
        }
    }

    // ---- 进度条悬停/拖动时间提示：鼠标在进度条上时，在指针位置显示对应时间 ----
    let seekTooltipVisible = false;
    function showSeekTooltip(clientX, slider) {
        if (!seekTooltip) return;
        const dur = currentDuration();
        if (!dur || dur <= 0) return;
        const rect = slider.getBoundingClientRect();
        if (!rect.width) return;
        const frac = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
        seekTooltip.textContent = formatTime(frac * dur);
        const left = Math.max(8, Math.min(window.innerWidth - seekTooltip.offsetWidth - 8, clientX - seekTooltip.offsetWidth / 2));
        seekTooltip.style.left = left + 'px';
        seekTooltip.style.top = Math.max(6, rect.top - 36) + 'px';
        seekTooltip.style.display = 'block';
        seekTooltipVisible = true;
    }
    function hideSeekTooltip() {
        if (seekTooltip) seekTooltip.style.display = 'none';
        seekTooltipVisible = false;
    }
    function bindSeekTooltip(slider) {
        if (!slider) return;
        slider.addEventListener('pointermove', (e) => showSeekTooltip(e.clientX, slider));
        slider.addEventListener('pointerdown', (e) => showSeekTooltip(e.clientX, slider));
        slider.addEventListener('input', () => {
            if (!seekTooltipVisible) return;
            const dur = currentDuration();
            if (dur > 0) {
                const p = parseFloat(slider.value) / 1000;
                seekTooltip.textContent = formatTime(p * dur);
            }
        });
        slider.addEventListener('pointerleave', () => {
            if (draggingProgress || draggingVideoProgress) return;
            hideSeekTooltip();
        });
    }
    bindSeekTooltip(progressBar);
    bindSeekTooltip(videoProgressBar);

// 主进度条
    if (progressBar) {
        progressBar.addEventListener('pointerdown', (e) => {
            draggingProgress = true;
            // 把指针事件锁定到滑块上，保证在滑块外松开也能收到 pointerup，
            // 避免 draggingProgress 卡在 true 导致播放时进度条不再刷新。
            try { progressBar.setPointerCapture(e.pointerId); } catch (_) {}
        });
        progressBar.addEventListener('pointerup', () => {
            // input 已触发时用暂存的最新值立即 seek；
            // 未触发说明 WebKitGTK 还没更新 value，此时不读旧值，等 change。
            if (pendingProgressPercent != null) commitProgressSeek();
            else draggingProgress = false;
        });
        progressBar.addEventListener('pointercancel', () => {
            draggingProgress = false;
            pendingProgressPercent = null;
        });
        // change 在 value 提交后（input 之后）触发，值是可靠的最终值。
        progressBar.addEventListener('change', commitProgressSeek);
        progressBar.addEventListener('input', () => {
            // 仅实时预览 + 暂存最新值，实际 Seek 在 change/pointerup 时执行
            const dur = currentDuration();
            if (dur > 0) {
                const p = parseFloat(progressBar.value) / 1000;
                pendingProgressPercent = p;
                currentTimeEl.textContent = formatTime(p * dur);
            }
        });
    }

// 视频底部进度条
    if (videoProgressBar) {
        videoProgressBar.addEventListener('pointerdown', (e) => {
            draggingVideoProgress = true;
            try { videoProgressBar.setPointerCapture(e.pointerId); } catch (_) {}
        });
        videoProgressBar.addEventListener('pointerup', () => {
            if (pendingVideoPercent != null) commitVideoSeek();
            else draggingVideoProgress = false;
        });
        videoProgressBar.addEventListener('pointercancel', () => {
            draggingVideoProgress = false;
            pendingVideoPercent = null;
        });
        videoProgressBar.addEventListener('change', commitVideoSeek);
        videoProgressBar.addEventListener('input', () => {
            // 仅更新显示，实际 Seek 在 change/pointerup 时执行
            const dur = currentDuration();
            if (dur > 0) {
                const p = parseFloat(videoProgressBar.value) / 1000;
                pendingVideoPercent = p;
                if (videoCurrentEl) videoCurrentEl.textContent = formatTime(p * dur);
            }
        });
    }

    // 音量：默认 40%（不要一打开就很大声），记住上次使用的音量；
    // 点击喇叭可静音，再次点击恢复上次音量。
    if (volumeSlider) {
        const applyStoredVolume = (saved) => {
            let v = 40;
            if (saved != null && !isNaN(Number(saved))) {
                v = Math.max(0, Math.min(100, Number(saved)));
            }
            volumeSlider.value = v;
            if (v > 0) lastVolume = v;
            updateVolumeUI(v);
        };
        invoke('store_get', { key: 'volume' })
            .then(applyStoredVolume)
            .catch(() => applyStoredVolume(null));

        volumeSlider.addEventListener('input', () => {
            const v = parseFloat(volumeSlider.value);
            if (v > 0) lastVolume = v;
            updateVolumeUI(v);
            invoke('store_set', { key: 'volume', value: v }).catch(() => {});
        });

        if (volumeIcon) {
            volumeIcon.addEventListener('click', () => {
                const cur = parseFloat(volumeSlider.value) || 0;
                if (cur > 0) {
                    lastVolume = cur;
                    volumeSlider.value = 0;
                    updateVolumeUI(0);
                    invoke('store_set', { key: 'volume', value: 0 }).catch(() => {});
                } else {
                    const v = lastVolume > 0 ? lastVolume : 40;
                    volumeSlider.value = v;
                    updateVolumeUI(v);
                    invoke('store_set', { key: 'volume', value: v }).catch(() => {});
                }
            });
        }
    }

    // ---- 鼠标滚轮调节音量（窗口任意位置，除播放列表/进度条） ----
    function wheelAdjustVolume(e) {
        // 播放列表需要滚动，不劫持；进度条滑块也放行，避免误改音量
        if (e.target.closest('#playlist-panel')) return;
        if (e.target.closest('.progress-slider')) return;
        e.preventDefault();
        const step = e.deltaY < 0 ? 5 : -5;
        let v = parseFloat(volumeSlider.value) + step;
        v = Math.max(0, Math.min(100, v));
        volumeSlider.value = v;
        updateVolumeUI(v);
    }
    document.addEventListener('wheel', wheelAdjustVolume, { passive: false });

    // ---- 媒体事件（audio 和 video 共用） ----
    function bindMediaEvents(media) {
        const tag = media === videoPlayer ? 'video' : 'audio';
        // 每个关键媒体事件都带时间戳记入日志，卡死点/卡死前一刻一目了然。
        const logMediaEvent = (name, extra = '') => {
            const seek = media.seekable && media.seekable.length
                ? `start=${Math.floor(media.seekable.start(0))},end=${Math.floor(media.seekable.end(media.seekable.length - 1))}`
                : 'empty';
            sendLog(`[${tag}] ${name} t=${media.currentTime?.toFixed(2)} dur=${media.duration ?? 'NaN'} ` +
                `readyState=${media.readyState} networkState=${media.networkState} seekable=${seek} ${extra}`);
        };

        media.addEventListener('timeupdate', () => {
            if (draggingProgress) return; // 拖动中不更新进度条值（但当前时间仍可更新）
            const dur = currentDuration();
            if (dur > 0) {
                const percent = Math.min((media.currentTime / dur) * 1000, 1000);
                progressBar.value = percent;
                currentTimeEl.textContent = formatTime(media.currentTime);
                totalTimeEl.textContent = formatTime(dur);
                if (media === videoPlayer && videoProgressBar) {
                    videoProgressBar.value = percent;
                    if (videoCurrentEl) videoCurrentEl.textContent = formatTime(media.currentTime);
                    if (videoTotalEl) videoTotalEl.textContent = formatTime(dur);
                }
                emitMiniProgress();
            }
        });

        media.addEventListener('ended', () => {
            // 记录日志（可选）
            logMediaEvent('ended');

            // 如果列表为空，停止播放并更新 UI
            if (playlist.length === 0) {
                isPlaying = false;
                updatePlayToggleIcon();
                setStatus('⏹ 播放结束', 'info');
                emitMiniState();
                return;
            }

            // 单曲循环模式（playMode === 1）：重新播放当前曲目
            if (playMode === 1) {
                // 直接重新播放当前索引（会重新加载并从头播放）
                playIndex(currentIndex);
                return;
            }

            // 其他模式（列表循环或随机）：播放下一首
            // 注意：playNext 内部会处理随机模式，并更新 currentIndex
            playNext();
        });

        media.addEventListener('error', (e) => {
            console.error('[Renderer] 播放错误:', e);
            const error = media.error;
            let errorMsg = '播放失败';
            if (error) {
                switch (error.code) {
                    case MediaError.MEDIA_ERR_ABORTED: errorMsg = '加载中止'; break;
                    case MediaError.MEDIA_ERR_NETWORK: errorMsg = '网络错误'; break;
                    case MediaError.MEDIA_ERR_DECODE: errorMsg = '解码错误'; break;
                    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED: errorMsg = '不支持的格式'; break;
                    default: errorMsg = '未知错误';
                }
            }
            logMediaEvent('error', `code=${error?.code ?? 'null'} msg="${errorMsg}" src=${media.currentSrc}`);

            setStatus(`❌ ${errorMsg}，尝试下一首`, 'error');
            isPlaying = false;
            updatePlayToggleIcon();

            if (playlist.length > 0 && currentIndex !== -1) {
                setTimeout(() => playNext(), 500);
            }
        });

        // 网络缓冲不足/加载被阻塞时都会触发。
        media.addEventListener('stalled', () => logMediaEvent('stalled'));
        media.addEventListener('waiting', () => logMediaEvent('waiting'));
        media.addEventListener('seeking', () => logMediaEvent('seeking'));
        //media.addEventListener('seeked', () => logMediaEvent('seeked'));
        //media.addEventListener('durationchange', () => logMediaEvent('durationchange'));
        media.addEventListener('emptied', () => logMediaEvent('emptied'));


        media.addEventListener('seeked', () => {
            logMediaEvent('seeked');
            const dur = currentDuration();
            if (dur > 0) {
                updateSeekUI(media, media.currentTime, dur);
            }
        });

        media.addEventListener('durationchange', () => {
            logMediaEvent('durationchange');
            // const newDur = media.duration;
            // if (isFiniteDuration(newDur) && newDur > 0) {
            //     // 如果 fixedDuration 为 0 或差异大于 0.5 秒，则更新
            //     if (fixedDuration === 0 || Math.abs(newDur - fixedDuration) > 0.5) {
            //         fixedDuration = newDur;
            //         totalTimeEl.textContent = formatTime(fixedDuration);
            //         if (media === videoPlayer && videoTotalEl) {
            //             videoTotalEl.textContent = formatTime(fixedDuration);
            //         }
            //         progressBar.disabled = false;
            //         progressBar.style.opacity = '1';
            //         if (videoProgressBar) {
            //             videoProgressBar.disabled = false;
            //             videoProgressBar.style.opacity = '1';
            //         }
            //         if (currentIndex >= 0 && playlist[currentIndex]) {
            //             playlist[currentIndex].duration = fixedDuration;
            //             updatePlaylistUI();
            //         }
            //         emitMiniState();
            //     }
            //     // 否则忽略微小变化
            // } else {
            //     // 无效时长不更新
            // }
        });


        media.addEventListener('play', () => {
            isPlaying = true;
            updatePlayToggleIcon();
            videoWrap.classList.remove('paused');
            if (media === videoPlayer && videoPlayBtn) videoPlayBtn.textContent = '⏸';
            videoControlsWake();
            emitMiniState();
        });
        media.addEventListener('pause', () => {
            isPlaying = false;
            updatePlayToggleIcon();
            if (media === videoPlayer && videoPlayBtn) videoPlayBtn.textContent = '▶';
            if (media === videoPlayer) {
                videoWrap.classList.add('paused');
                videoWrap.classList.remove('idle');
            }
            emitMiniState();
        });

        media.addEventListener('loadedmetadata', () => {
            logMediaEvent('loadedmetadata');
            const dur = media.duration;
            if (isFiniteDuration(dur) && dur > 0) {
                // 如果 playlist 中 duration 为 0，则更新（避免首次加载无数据）
                if (currentIndex >= 0 && playlist[currentIndex] && !playlist[currentIndex].duration) {
                    playlist[currentIndex].duration = dur;
                    updatePlaylistUI();
                }
            }
            // 使用 currentDuration() 更新显示
            const totalDur = currentDuration();
            if (totalDur > 0) {
                totalTimeEl.textContent = formatTime(totalDur);
                if (media === videoPlayer && videoTotalEl) {
                    videoTotalEl.textContent = formatTime(totalDur);
                }
                progressBar.disabled = false;
                progressBar.style.opacity = '1';
                if (videoProgressBar) {
                    videoProgressBar.disabled = false;
                    videoProgressBar.style.opacity = '1';
                }
            } else {
                totalTimeEl.textContent = '--:--';
                // 进度条可暂时禁用
            }
            emitMiniState();
        });

        media.addEventListener('canplay', () => {
            logMediaEvent('canplay');
        });
    }

    if (audioPlayer) bindMediaEvents(audioPlayer);
    if (videoPlayer) bindMediaEvents(videoPlayer);

    // ---- 键盘快捷键 ----
    document.addEventListener('keydown', (e) => {
        if (e.target.tagName === 'INPUT') return;
        if (e.key === ' ' || e.code === 'Space') {
            e.preventDefault();
            togglePlayPause();
        }
        if (e.key === 'ArrowRight') {
            e.preventDefault();
            const media = activeMedia();
            if (media && currentDuration() > 0) {
                media.currentTime = clampSeekTarget(media, media.currentTime + 5);
            }
        }
        if (e.key === 'ArrowLeft') {
            e.preventDefault();
            const media = activeMedia();
            if (media) {
                media.currentTime = Math.max(media.currentTime - 5, 0);
            }
        }
        if (e.key === 'Escape') {
            if (inContentFullscreen) {
                exitContentFullscreen();
            }
            closePlaylist();
        }
    });

    // ============================================================
    // 7. 初始化
    // ============================================================
    updatePlaylistUI();
    updateSongInfo(null);
    updatePlayToggleIcon();
    updateCover(null);
    updateModeBtn();
    setStatus('✅ 就绪，选择音乐文件或导入文件夹开始播放', 'success');
    // 加载主题
    loadTheme();
    console.log('[Renderer] 初始化完成');
});

if (document.readyState === 'loading') {
    console.log('[Renderer] 等待 DOM 加载...');
} else {
    console.log('[Renderer] DOM 已加载（事件已触发）');
}
