// 外层包裹，捕捉所有潜在崩溃
try {
    const tauri = window.__TAURI__;
    const invoke = tauri ? tauri.core.invoke : null;
    const getCurrentWindow = tauri ? tauri.window.getCurrentWindow : null;
    const emit = tauri ? tauri.event.emit : null;

    const closeBtn = document.getElementById('close-settings-btn');
    const autoCard = document.getElementById('auto-card');
    const autoPreview = document.getElementById('auto-preview');
    const monthGrid = document.getElementById('month-grid');
    const customCard = document.getElementById('custom-card');
    const applyCustomBtn = document.getElementById('apply-custom-btn');
    const cBg = document.getElementById('c-bg');
    const cPrimary = document.getElementById('c-primary');
    const cSecondary = document.getElementById('c-secondary');
    const cAccent = document.getElementById('c-accent');
    const cText = document.getElementById('c-text');

    if (!closeBtn || !monthGrid) {
        throw new Error('设置页 DOM 元素缺失，请检查 settings.html');
    }

    const MONTH_NAMES = ['一月','二月','三月','四月','五月','六月','七月','八月','九月','十月','十一月','十二月'];
    let currentTheme = 'auto';

    function setRootVars(p) {
        const root = document.documentElement;
        const c = Theme.paletteToApp(p);
        root.style.setProperty('--bg-primary', c.bg);
        root.style.setProperty('--bg-secondary', c.bgSecondary);
        root.style.setProperty('--text-primary', c.text);
        root.style.setProperty('--text-secondary', c.textSecondary);
        root.style.setProperty('--accent', c.accent);
        root.style.setProperty('--accent-hover', c.accentHover);
        root.style.setProperty('--border-color', c.border);
        root.style.setProperty('--card-bg', c.cardBg);
    }

    function buildMonthGrid() {
        monthGrid.innerHTML = '';
        Theme.MONTH_THEMES.forEach((t, i) => {
            const card = document.createElement('div');
            card.className = 'month-card';
            card.dataset.theme = 'month-' + (i + 1);

            const dots = document.createElement('div');
            dots.className = 'month-dots';
            [t.bg, t.accent, t.primary, t.text].forEach(color => {
                const s = document.createElement('span');
                s.style.background = color;
                dots.appendChild(s);
            });

            const name = document.createElement('span');
            name.className = 'month-name';
            name.textContent = MONTH_NAMES[i];

            card.appendChild(dots);
            card.appendChild(name);
            card.addEventListener('click', () => saveTheme('month-' + (i + 1)));
            monthGrid.appendChild(card);
        });
    }

    function refreshSelection() {
        const cards = [autoCard, customCard, ...Array.from(monthGrid.querySelectorAll('.month-card'))];
        cards.forEach(c => {
            c.classList.toggle('active', c.dataset.theme === currentTheme);
        });
        // 设置页底色跟随当前主题
        if (currentTheme === 'custom') {
            setRootVars({
                primary: cPrimary.value, secondary: cSecondary.value,
                bg: cBg.value, accent: cAccent.value, text: cText.value
            });
        } else {
            Theme.resolvePalette(currentTheme).then(setRootVars);
        }
    }

    function updateAutoPreview() {
        const m = Theme.currentMonth();
        document.getElementById('current-month').textContent = MONTH_NAMES[m - 1];
        const t = Theme.MONTH_THEMES[m - 1];
        autoPreview.style.background = `linear-gradient(135deg, ${t.bg}, ${t.accent})`;
        autoPreview.textContent = '';
        autoPreview.innerHTML = '<span style="font-size:26px">🎨</span>';
        autoPreview.style.color = t.text;
    }

    async function saveTheme(theme, custom = null) {
        currentTheme = theme;
        if (invoke) {
            await invoke('store_set', { key: 'theme', value: theme });
            if (custom) {
                await invoke('store_set', { key: 'customTheme', value: custom });
            }
        }
        refreshSelection();
        if (emit) {
            emit('theme:changed', theme);
        }
    }

    async function loadSettings() {
        buildMonthGrid();
        updateAutoPreview();
        if (!invoke) return;
        const theme = await invoke('store_get', { key: 'theme' }) || 'auto';
        currentTheme = theme;

        const custom = await invoke('store_get', { key: 'customTheme' }) || {};
        cBg.value = custom.bg || Theme.MONTH_THEMES[0].bg;
        cPrimary.value = custom.primary || Theme.MONTH_THEMES[0].primary;
        cSecondary.value = custom.secondary || Theme.MONTH_THEMES[0].secondary;
        cAccent.value = custom.accent || Theme.MONTH_THEMES[0].accent;
        cText.value = custom.text || Theme.MONTH_THEMES[0].text;

        // 老版本主题名（dawn/forest/dusk）自动回落为“跟随月份”
        if (!['auto', 'custom'].includes(theme) && !theme.startsWith('month-')) {
            currentTheme = 'auto';
        }
        refreshSelection();
    }

    closeBtn.addEventListener('click', () => {
        if (emit) emit('settings:closed');
        if (getCurrentWindow) getCurrentWindow().close();
        else window.close();
    });

    autoCard.addEventListener('click', () => saveTheme('auto'));

    applyCustomBtn.addEventListener('click', async () => {
        const custom = {
            bg: cBg.value,
            primary: cPrimary.value,
            secondary: cSecondary.value,
            accent: cAccent.value,
            text: cText.value,
        };
        await saveTheme('custom', custom);
    });

    loadSettings();
    console.log('[Settings] 设置页初始化完成');

} catch (error) {
    console.error('[Settings] 脚本执行出错:', error.message);
    console.error(error.stack);
}
