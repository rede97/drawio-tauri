// Electron → Tauri bridge
// Injected via with_initialization_script() before webapp loads.
// Makes draw.io detect as desktop app and maps electron.* API to Tauri IPC.

(function() {
	var _ua = navigator.userAgent;
	Object.defineProperty(navigator, 'userAgent', {
		get: function() { return _ua + ' electron/29.7.11 draw.io/29.7.11'; }
	});

	window.process = { versions: { electron: 'tauri' } };

	var invoke = window.__TAURI_INTERNALS__.invoke;
	var _listeners = {};
	var _onceListeners = {};

	function _emit(channel, data) {
		(_listeners[channel] || []).forEach(function(cb) { try { cb(data); } catch(e) {} });
		if (_onceListeners[channel]) {
			_onceListeners[channel].forEach(function(cb) { try { cb(data); } catch(e) {} });
			delete _onceListeners[channel];
		}
	}

	function _normalizeFilters(filters) {
		if (!filters) return [];
		return filters.map(function(f) {
			var exts = (f.extensions || []).map(function(e) {
				return e.replace(/^\*\./, '');
			});
			return { name: f.name, extensions: exts };
		});
	}

	window.electron = {
		request: function(msg, success, error) {
			if (typeof msg === 'string') msg = { action: msg };

			// Clipboard actions handled via navigator.clipboard (webview native)
			if (msg.action === 'clipboardAction') {
				if (msg.clipboardAction === 'readText') {
					navigator.clipboard.readText()
						.then(function(t) { if (success) success(t); })
						.catch(function(e) { if (error) error(e); });
				} else if (msg.clipboardAction === 'writeText') {
					navigator.clipboard.writeText(msg.data || '')
						.then(function() { if (success) success(null); })
						.catch(function(e) { if (error) error(e); });
				} else {
					if (error) error('Unsupported clipboard action: ' + msg.clipboardAction);
				}
				return;
			}

			// All other actions go to Rust
			invoke('electron_request', { msg: msg })
				.then(function(r) { if (success) success(r); })
				.catch(function(e) {
					if (error) error(typeof e === 'string' ? e : (e.message || 'Unknown error'), e);
				});
		},

		sendMessage: function(channel, data) {
			invoke('electron_message', { channel: channel, data: data || {} })
				.catch(function() {});
		},

		registerMsgListener: function(channel, callback) {
			if (!_listeners[channel]) _listeners[channel] = [];
			_listeners[channel].push(callback);
		},

		listenOnce: function(channel, callback) {
			if (!_onceListeners[channel]) _onceListeners[channel] = [];
			_onceListeners[channel].push(callback);
		}
	};

	// Intercept postMessage to catch Tauri events (Rust → JS push)
	var _origPM = window.__TAURI_INTERNALS__.postMessage;
	window.__TAURI_INTERNALS__.postMessage = function(msg) {
		if (msg && msg.event && typeof msg.event === 'string') {
			_emit(msg.event, msg.payload);
		}
		return _origPM.apply(this, arguments);
	};
})();
