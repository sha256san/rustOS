# rust_os 仕様書 兼 独立型 APT サーバー構築計画書

本ドキュメントは、`rust_os` (Ubuntu 26.04 LTS Server エミュレーション) のシステムアーキテクチャ・通信規格と、別環境で動作させる **独立型 APT リポジトリ / プロキシサーバー (`aptserver`) の構築計画** を一元的にまとめた統合設計書です。

---

## 第1部: rust_os システム仕様 & アーキテクチャ

### 1.1 OS 基本情報

| 項目 | 内容 |
|:---|:---|
| **OS名** | `rust_os` (Ubuntu 26.04 LTS Server Resolute Raccoon エミュレーション) |
| **アーキテクチャ** | `x86_64` (Long Mode, 64-bit) |
| **カーネル** | Rust 製 `no_std` ベアメタルカーネル |
| **ブート方式** | Legacy BIOS (MBR ディスクイメージ `.vmdk` / `.img`) および UEFI |
| **ファイルシステム** | 仮想ファイルシステム (VFS) + メモリ内 initramfs (`initfs.tar`) |
| **標準ツール** | `sudo-rs (v0.2.1)`、`systemd` サービス管理、`uutils/coreutils` |

---

### 1.2 パッケージ管理システム (APT) の動作原理

`rust_os` 内の `apt` コマンド (`apt update`, `apt install`, `apt remove` 等) は、オンラインの外部 APT サーバー (**`http://153.125.239.34:8081`** および COM2 シリアル: **`153.125.239.34:12345`**) と通信を行います。

1. **`apt update`**:
   - オンラインの APT サーバー (`http://153.125.239.34:8081/Packages.json`) にリクエスト。
   - レスポンスの JSON をパースし、利用可能パッケージ名・バージョン・サイズ・依存関係 (`depends`) をインメモリ DB に同期。
2. **`apt install <package_name>`**:
   - 依存関係 (`depends`) を再帰解析し、必要な全パッケージをリスト化。
   - VFS 内 (`/usr/share/apt/unpacked/<name>`) にファイルがない場合、サーバーへ `http://153.125.239.34:8081/packages/<name>.tar` を要求。
   - 返却された tar アーカイブを受信し、VFS 内の `/usr/bin/` などの実パスへ自動解凍・配置。

---

## 第2部: 通信プロトコル & API 仕様 (API Spec)

APT サーバーは **HTTP サーバー (`153.125.239.34:8081`)** または **COM2 シリアル TCP プロキシ (`153.125.239.34:12345`)** として動作し、以下の規格で応答します。

### 2.1 シリアル / TCP プロトコル電文規格

| 種別 | フォーマット | 説明 |
|:---|:---|:---|
| **リクエスト** | `REQ:GET:<path>\n` | パス指定の読み込み要求（例: `REQ:GET:/Packages.json\n`） |
| **レスポンス (成功)** | `RES:OK:<data_bytes>\n<binary_data>` | ヘッダーにバイト数を付与し、続けてバイナリデータを送信 |
| **レスポンス (エラー)** | `RES:ERROR:<message>\n` | エラー理由テキストを返却 |

### 2.2 エンドポイント仕様

#### ① `/Packages.json` (GET)
- **用途**: リポジトリ全体のパッケージメタデータ提供
- **形式**: JSON 配列
```json
[
  {
    "name": "neofetch",
    "version": "7.1.0-4",
    "size": "82 KB",
    "depends": []
  },
  {
    "name": "curl",
    "version": "7.88.1-10",
    "size": "198 KB",
    "depends": ["libcurl4"]
  },
  {
    "name": "libcurl4",
    "version": "7.88.1-10",
    "size": "290 KB",
    "depends": []
  },
  {
    "name": "git",
    "version": "2.39.2-1",
    "size": "3.2 MB",
    "depends": ["git-man"]
  },
  {
    "name": "python3",
    "version": "3.11.2-1",
    "size": "24 KB",
    "depends": ["python3-minimal"]
  }
]
```

#### ② `/packages/<package_name>.tar` (GET)
- **用途**: 指定パッケージの実体バイナリを含む tar アーカイブ配信
- **展開対象パス例**: `usr/bin/neofetch`, `usr/bin/curl`, `usr/bin/python3`
- **形式**: tar アーカイブバイナリ

---

## 第3部: 独立型 APT サーバー (`aptserver`) 構築計画書

別端末・クラウド・Docker 環境等で独立動作する APT サーバーの構築計画です。

### 3.1 ディレクトリ・ファイル構成

```
aptserver/
├── server.py              # [新規] HTTP (8080) & TCP シリアル (12345) 統合サーバー
├── fetch_pkg.py           # Debian/Ubuntu 公式プールからの自動キャッシュ・変換スクリプト
├── Dockerfile             # [新規] 独立コンテナ構築用 Dockerfile
├── docker-compose.yml     # [新規] ワンコマンド起動構成
└── cache/                 # 取得済みパッケージキャッシュ
    ├── Packages.json
    ├── neofetch.tar
    ├── curl.tar
    └── git.tar
```

---

### 3.2 サーバー実装コンポーネント計画

#### 1. 統合サーバープログラム (`aptserver/server.py`)
- Python 標準ライブラリ (`http.server`, `socketserver`, `threading`) のみで記述し、外部ライブラリ依存ゼロで動作。
- **HTTP サーバー スレッド (Port 8080)**:
  - `/Packages.json` へのアクセスに対し、`cache/Packages.json` を返答。
  - `/packages/<name>.tar` へのアクセスに対し、`cache/<name>.tar` を配信。
- **TCP シリアルプロキシ スレッド (Port 12345)**:
  - VMware / QEMU の COM2 仮想シリアル接続を受け付け。
  - `REQ:GET:<path>` を受信後、該当するファイルを `RES:OK:<bytes>\n<data>` フォーマットで送信。

#### 2. 公式パッケージ自動キャッシュスクリプト (`aptserver/fetch_pkg.py`)
- Debian / Ubuntu 公式リポジトリ (Debian pool) から指定パッケージ (`neofetch`, `curl`, `git`, `python3`, `tree` 等) の最新 `.deb` をダウンロード。
- `.deb` 内の `data.tar` を抽出して `cache/<name>.tar` に保存し、`cache/Packages.json` を自動動的生成。

#### 3. コンテナ化 & 起動スクリプト (`Dockerfile` & `docker-compose.yml`)
- Linux / Windows / macOS / クラウドサーバー上で `docker-compose up -d` を打つだけで完全起動する環境を提供。

---

## 第4部: 検証・テスト計画

### 4.1 単体動作テスト
1. **HTTP エンドポイントテスト**:
   `curl -i http://localhost:8080/Packages.json` で HTTP 200 OK および JSON データを取得できるか確認。
2. **シリアル TCP プロトコルテスト**:
   `python3 -c "import socket; s=socket.socket(); s.connect(('127.0.0.1', 12345)); s.sendall(b'REQ:GET:/Packages.json\n'); print(s.recv(1024))"` で `RES:OK:...` が返ることを確認。

### 4.2 統合テスト
- 独立サーバーを起動した状態で `rust_os` から `apt update` および `apt install neofetch` を実行し、全自動でダウンロード・展開が完了することを検証。
