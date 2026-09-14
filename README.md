# rust_os

Ubuntu 26.04 LTS Server ("Resolute Raccoon") の環境・アーキテクチャのエミュレーションを目指した、Rust製の `no_std` ベアメタル自作OSプロジェクトです。

## プロジェクトの目的

このプロジェクトは、既存のLinux/Ubuntuのシステムアーキテクチャ（ファイルシステム階層、パッケージ管理、初期化システムなど）をRustによるフルスクラッチOS上で再現し、「Ubuntu環境として違和感なく使えるOS」を構築することを目的としています。

## 現在のステータス (Current Status)

現在は初期構築フェーズ（フェーズ1）にあり、以下のコンポーネントが実装・設計されています。

* **ベアメタルカーネル (`rust_os/`)**:
    * Rustベースの `no_std` カーネル
    * 基本的なハードウェア初期化 (GDT, IDT, PIC)
    * VGAテキストバッファによる画面出力
    * メモリ内ファイルシステム (VFS) と `initfs.tar` を用いた初期RAMディスク機構
    * シリアル通信 (COM2) を経由したネットワーク通信のスタブ実装
* **独立型 APT サーバー (`aptserver/`)**:
    * Debian公式プールからパッケージ（`neofetch`, `curl`, `python3` 等）を動的にダウンロード・キャッシュするPythonスクリプト (`fetch_pkg.py`)
    * キャッシュしたtarアーカイブやパッケージリスト (`Packages.json`) を配信する専用サーバー実装
    * OS側の `apt` コマンドからのリクエスト（HTTP または シリアルTCPプロキシ経由）に応答する仕組み
* **各種設計ドキュメント**:
    * `計画書.md`: Ubuntu OS構成要素の実装計画
    * `requirements.md`: 実装要件定義チェックリスト
    * `rust_os_apt_architecture_spec.md`: 独立型APTサーバーおよび通信規格の仕様書
    * `ubuntu_26_04_lts_server_basic_guide.md`: エミュレーションターゲットであるUbuntu 26.04 LTSの基礎ガイド

## ディレクトリ構成

```text
.
├── aptserver/                            # 独立型APTリポジトリ/プロキシサーバー実装
│   ├── src/main.rs                       # APTサーバーのエントリーポイント
│   ├── fetch_pkg.py                      # Debianプールからのパッケージ取得スクリプト
│   └── cache/                            # ダウンロード済みパッケージキャッシュ
├── rust_os/                              # OSカーネル本体のソースコード
│   ├── src/
│   │   ├── main.rs                       # カーネルのエントリーポイント
│   │   ├── vga_buffer.rs                 # VGAテキスト出力
│   │   ├── vfs.rs                        # 仮想ファイルシステム
│   │   ├── apt.rs                        # OS側APTクライアント実装
│   │   └── ...                           # GDT, 割り込み等のハードウェア制御コード
│   ├── build/                            # ビルド済みディスクイメージ出力先
│   ├── create_disk.sh                    # 起動用ディスクイメージ作成スクリプト
│   └── Cargo.toml                        # カーネルの依存関係定義
├── generate_initfs.py                    # カーネルに組み込む初期ファイルシステム(initfs)の生成スクリプト
├── requirements.md                       # 実装要件定義書
├── rust_os_apt_architecture_spec.md      # APTアーキテクチャ仕様書
├── ubuntu_26_04_lts_server_basic_guide.md # Ubuntu 26.04 ターゲット仕様
└── 計画書.md                             # 開発計画・参照要素サマリ
```

## 開発・ビルド方法

（※ 詳細なビルド手順は今後整備予定です）
現状、カーネルのビルドにはRustのナイトリーツールチェーンと `bootimage` クレートが必要です。

```bash
cd rust_os
cargo run # または cargo bootimage
```

## アーキテクチャ図

カーネル内の `apt` コマンドが外部サーバーからパッケージを取得するアーキテクチャの概要です：

```mermaid
graph LR
    OS[rust_os (apt command)] -- COM2 Serial / TCP --> Server[aptserver (Port: 8081)]
    Server -- HTTP GET --> Debian[Debian Official Pool]
    Server -. Cache .-> CacheDir[aptserver/cache]
```
