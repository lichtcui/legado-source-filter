// Legado Java API polyfill for search URL generation
// Each source's searchUrl JS code runs in this context.
// Required globals (injected by Rust): __BASE_URL__, __KEYWORD__, __PAGE__

// ── java.* API ──
globalThis.java = {
    _store: {},
    _result: null,

    ajax: function(url) {
        // Signal to Rust that an HTTP request is needed
        // The polyfill itself can't make HTTP — Rust handles it
        console.log(JSON.stringify({ type: 'ajax', url: String(url) }));
        return '';
    },

    post: function(url, body, headers) {
        console.log(JSON.stringify({ type: 'post', url: String(url), body: String(body), headers: headers || {} }));
        return '';
    },

    put: function(key, value) {
        this._store[String(key)] = String(value);
    },

    get: function(key) {
        return this._store[String(key)] || '';
    },

    getString: function(path) {
        // java.getString('$.field') — evaluate JSONPath on stored result
        try {
            const stored = globalThis.__lastResponse || '';
            const parts = path.replace(/^\$\./, '').split('.');
            let obj = JSON.parse(stored);
            for (const p of parts) {
                if (p.includes('[')) {
                    const name = p.split('[')[0];
                    const idx = parseInt(p.split('[')[1]);
                    obj = name ? obj[name][idx] : obj[idx];
                } else {
                    obj = obj[p];
                }
            }
            return String(obj);
        } catch(e) {
            return '';
        }
    },

    t2s: function(text) {
        // Traditional to Simplified Chinese conversion stub
        return String(text);
    },

    longToast: function(msg) {
        console.error('[TOAST]', String(msg));
    },

    toast: function(msg) {
        console.error('[TOAST]', String(msg));
    },

    startBrowserAwait: function(url, msg) {
        // Cannot polyfill — returns a sentinel
        console.error('[BROWSER_AWAIT]', String(url), String(msg));
        // Signal to Rust that this is untestable
        console.log(JSON.stringify({ type: 'start_browser_await', url: String(url) }));
    },

    setContent: function(html) {
        globalThis.__lastResponse = String(html);
    },

    getElement: function(selector) {
        return [];
    }
};

// ── cookie.* API ──
globalThis.cookie = {
    _jar: {},
    removeCookie: function(key) {
        delete this._jar[String(key)];
    }
};

// ── source.* API ──
globalThis.source = {
    getKey: function() {
        return globalThis.__SOURCE_KEY__ || '';
    },
    getLoginHeader: function() {
        return globalThis.__LOGIN_HEADER__ || '';
    },
    getBookSourceComment: function() {
        return globalThis.__SOURCE_COMMENT__ || '';
    }
};

// ── cache.* API ──
globalThis.cache = {
    _data: {},
    get: function(key) {
        return this._data[String(key)];
    },
    put: function(key, value, ttl) {
        this._data[String(key)] = value;
    }
};

// ── Java built-in objects ──
globalThis.Packages = {
    java: {
        lang: {
            String: String,
            System: { currentTimeMillis: () => Date.now() }
        },
        text: { SimpleDateFormat: function() { return { format: (d) => String(d) }; } },
        net: { URLEncoder: { encode: (s) => encodeURIComponent(s) } }
    }
};

globalThis.JavaImporter = function() { return function() {}; };

// Result extraction is handled inline in runner.rs
