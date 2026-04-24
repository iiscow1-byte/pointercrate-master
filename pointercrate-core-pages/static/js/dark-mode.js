(function () {
    var isDark;

    function applyTheme(dark) {
        isDark = dark;
        document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
        localStorage.setItem('darkMode', dark ? '1' : '0');
        var icon = document.querySelector('#dark-mode-toggle i');
        if (icon) {
            icon.className = dark ? 'fas fa-sun' : 'fas fa-moon';
        }
    }

    var stored = localStorage.getItem('darkMode');
    var prefersDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    isDark = stored !== null ? stored === '1' : prefersDark;

    // Set theme immediately to prevent flash of wrong theme
    document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');

    document.addEventListener('DOMContentLoaded', function () {
        var icon = document.querySelector('#dark-mode-toggle i');
        if (icon) {
            icon.className = isDark ? 'fas fa-sun' : 'fas fa-moon';
        }
        var btn = document.getElementById('dark-mode-toggle');
        if (btn) {
            btn.addEventListener('click', function () {
                applyTheme(!isDark);
            });
        }
    });
})();
