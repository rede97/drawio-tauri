const fs = require('fs')
const path = require('path')

const appjsonpath = path.join(__dirname, 'package.json')
const versionPath = path.join(__dirname, 'drawio', 'VERSION')
const cargoTomlPath = path.join(__dirname, 'src-tauri', 'Cargo.toml')
const tauriConfPath = path.join(__dirname, 'src-tauri', 'tauri.conf.json')

if (!fs.existsSync(versionPath))
{
	console.error('Error: drawio/VERSION not found. Did you clone with --recursive or run git submodule update --init?')
	process.exit(1)
}

let ver = fs.readFileSync(versionPath, 'utf8').trim()

if (!/^\d+\.\d+\.\d+$/.test(ver))
{
	console.error('Error: drawio/VERSION contains invalid version: "' + ver + '"')
	process.exit(1)
}

// Update package.json version
let pj = require(appjsonpath)
pj.version = ver
fs.writeFileSync(appjsonpath, JSON.stringify(pj, null, 2), 'utf8')

// Update Cargo.toml version
if (fs.existsSync(cargoTomlPath))
{
	let cargo = fs.readFileSync(cargoTomlPath, 'utf8')
	cargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, 'version = "' + ver + '"')
	fs.writeFileSync(cargoTomlPath, cargo, 'utf8')
}

// Update tauri.conf.json version
if (fs.existsSync(tauriConfPath))
{
	let tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'))
	tauriConf.version = ver
	fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2), 'utf8')
}

console.log('Synced version to ' + ver)
