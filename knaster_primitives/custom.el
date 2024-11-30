;;; custom.el Set emacs Rust LSP settings -*- lexical-binding: t; -*-
;;
;; Copyright (C) 2024 Erik Natanael Gustafsson
;;
;;; Commentary:
;; This file contains commands for setting rust-analyzers active features in Doom Emacs, Rustic, LSP mode
;;
;;
;;; Code:

;; You need to set both to get inline error checking
(setq lsp-rust-features ["alloc"])
(setq lsp-rust-analyzer-checkonsave-features ["alloc"])
;; Then run `lsp-workspace-restart'


(provide 'custom)
;;; custom.el ends here
