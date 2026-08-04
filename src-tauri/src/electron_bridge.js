// Electron → Tauri bridge
// Injected via with_initialization_script() before webapp loads.
// Makes draw.io detect as desktop app and maps electron.* API to Tauri IPC.

window.DRAWIO_CONFIG = { enableLocalFonts: true };

// Clear stale IndexedDB drafts so splash screen always shows on startup
(function() {
	try {
		var req = indexedDB.open('database', 2);
		req.onsuccess = function(e) {
			var db = e.target.result;
			var tx = db.transaction(['objects'], 'readwrite');
			var store = tx.objectStore('objects');
			var cr = store.openCursor();
			cr.onsuccess = function(e) {
				var c = e.target.result;
				if (c) {
					if (c.key.toString().indexOf('.draft_') === 0) { c.delete(); }
					c.continue();
				}
			};
		};
	} catch(e) {}
})();

(function() {
	var _ua = navigator.userAgent;
	Object.defineProperty(navigator, 'userAgent', {
		get: function() { return _ua + ' electron/tauri draw.io/__DRAWIO_VERSION__'; }
	});

	window.process = { versions: { electron: 'tauri' } };

	var invoke = window.__TAURI_INTERNALS__.invoke;
	var _listeners = {};
	var _onceListeners = {};
	var _fileWatchListeners = {};

	function _emit(channel, data) {
		(_listeners[channel] || []).forEach(function(cb) { try { cb(data); } catch(e) {} });
		if (_onceListeners[channel]) {
			_onceListeners[channel].forEach(function(cb) { try { cb(data); } catch(e) {} });
			delete _onceListeners[channel];
		}
	}

	// file-watch-changed event: dispatch to registered per-path listeners
	_listeners['file-watch-changed'] = [function(payload) {
		var path = payload && payload.path;
		var cb = path && _fileWatchListeners[path];
		if (cb) {
			try { cb(payload.curr, payload.prev); } catch(e) {}
		}
	}];

	function _dataUrlToBlob(dataUrl) {
		var parts = dataUrl.split(',');
		var mime = parts[0].match(/:(.*?);/)[1];
		var raw = atob(parts[1]);
		var bytes = new Uint8Array(raw.length);
		for (var i = 0; i < raw.length; i++) { bytes[i] = raw.charCodeAt(i); }
		return new Blob([bytes], { type: mime });
	}

	function _blobToDataUrl(blob, cb) {
		var reader = new FileReader();
		reader.onload = function() { cb(null, reader.result); };
		reader.onerror = function(e) { cb(e); };
		reader.readAsDataURL(blob);
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

	// Bridge functions callable from Rust via window.eval()
	// Each maps to _emit() which dispatches to registered listeners.
	// Close handshake mirrors drawio-desktop: the webapp registers
	// isModified / saveAndClose / removeDraft listeners in ElectronApp.js.
	window.__tauriCloseCheck = function(uniqueId) { _emit('isModified', uniqueId); };
	window.__tauriSaveAndClose = function(uniqueId) { _emit('saveAndClose', uniqueId); };
	window.__tauriRemoveDraft = function() { _emit('removeDraft', {}); };
	window.__tauriFileChanged = function(data) { _emit('file-watch-changed', data); };
	window.__tauriArgsObj = function(data) { _emit('args-obj', data); };
	window.__tauriExportError = function(data) { _emit('export-error', data); };

	window.electron = {
		request: function(msg, success, error) {
			if (typeof msg === 'string') msg = { action: msg };

			// Clipboard actions handled via navigator.clipboard (webview native)
			if (msg.action === 'clipboardAction') {
				var method = msg.method || msg.clipboardAction;

				if (method === 'readText') {
					navigator.clipboard.readText()
						.then(function(t) { if (success) success(t); })
						.catch(function(e) { if (error) error(e); });
				} else if (method === 'writeText') {
					navigator.clipboard.writeText(msg.data || '')
						.then(function() { if (success) success(null); })
						.catch(function(e) { if (error) error(e); });
				} else if (method === 'writeImage') {
					try {
						var imgData = msg.data || {};
						var blob = _dataUrlToBlob(imgData.dataUrl);
						navigator.clipboard.write([new ClipboardItem((_b = {}, _b[blob.type] = blob, _b))])
							.then(function() { if (success) success(null); })
							.catch(function(e) { if (error) error(e); });
						var _b;
					} catch(e) {
						if (error) error(e.message || 'writeImage failed');
					}
				} else if (method === 'readImage') {
					try {
						navigator.clipboard.read()
							.then(function(items) {
								for (var i = 0; i < items.length; i++) {
									for (var j = 0; j < items[i].types.length; j++) {
										var t = items[i].types[j];
										if (t.indexOf('image/') === 0) {
											return items[i].getType(t).then(function(blob) {
												_blobToDataUrl(blob, function(err, dataUrl) {
													if (err) { if (error) error(err); return; }
													if (success) success(dataUrl);
												});
											});
										}
									}
								}
								if (success) success(null);
							})
							.catch(function(e) { if (error) error(e); });
					} catch(e) {
						if (error) error(e.message || 'readImage failed');
					}
				} else {
					if (error) error('Unsupported clipboard action: ' + method);
				}
				return;
			}

			// watchFile: store listener JS-side, then forward to Rust (without listener)
			if (msg.action === 'watchFile') {
				if (msg.listener) {
					_fileWatchListeners[msg.path] = msg.listener;
				}
				invoke('electron_request', { msg: { action: 'watchFile', path: msg.path } })
					.then(function(r) { if (success) success(r); })
					.catch(function(e) {
						if (error) error(typeof e === 'string' ? e : (e.message || 'Unknown error'), e);
					});
				return;
			}

			// unwatchFile: remove listener JS-side, then forward to Rust
			if (msg.action === 'unwatchFile') {
				delete _fileWatchListeners[msg.path || msg.file];
				invoke('electron_request', { msg: { action: 'unwatchFile', path: msg.path || msg.file } })
					.then(function(r) { if (success) success(r); })
					.catch(function(e) {
						if (error) error(typeof e === 'string' ? e : (e.message || 'Unknown error'), e);
					});
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

	// Spellcheck: override draw.io's spellcheck="false" on text elements when enabled
	(function() {
		var search = window.location.search;
		if (search.indexOf('enableSpellCheck=1') !== -1 || search.indexOf('enableSpellCheck=1&') !== -1 || search.indexOf('&enableSpellCheck=1') !== -1) {
			function enableSpellcheck(el) {
				el.setAttribute('spellcheck', 'true');
			}
			function scan(root) {
				if (root.nodeType === 1) {
					if (root.matches && root.matches('input, textarea, [contenteditable]')) {
						enableSpellcheck(root);
					}
					var list = root.querySelectorAll && root.querySelectorAll('input, textarea, [contenteditable]');
					if (list) {
						for (var i = 0; i < list.length; i++) { enableSpellcheck(list[i]); }
					}
				}
			}
			document.addEventListener('DOMContentLoaded', function() {
				var observer = new MutationObserver(function(mutations) {
					for (var i = 0; i < mutations.length; i++) {
						var added = mutations[i].addedNodes;
						for (var j = 0; j < added.length; j++) { scan(added[j]); }
					}
				});
				observer.observe(document.documentElement, { childList: true, subtree: true });
				scan(document.documentElement);
			});
		}
	})();
})();
