local M = {}

function M.setup()
  local lspconfig = require("lspconfig")
  local configs = require("lspconfig.configs")
  local util = require("lspconfig.util")

  if not configs.trust_lsp then
    configs.trust_lsp = {
      default_config = {
        cmd = { "trust-lsp" },
        filetypes = { "st", "pou" },
        root_dir = function(fname)
          return util.root_pattern("trust-lsp.toml", ".git")(fname)
            or util.path.dirname(fname)
        end,
        single_file_support = true,
      },
    }
  end

  lspconfig.trust_lsp.setup({
    on_attach = function(_, bufnr)
      local opts = { buffer = bufnr, silent = true }
      vim.bo[bufnr].omnifunc = "v:lua.vim.lsp.omnifunc"
      vim.keymap.set("n", "K", vim.lsp.buf.hover, opts)
      vim.keymap.set("n", "gd", vim.lsp.buf.definition, opts)
      vim.keymap.set("n", "gr", vim.lsp.buf.references, opts)
      vim.keymap.set("n", "<leader>f", function()
        vim.lsp.buf.format({ async = false })
      end, opts)
    end,
  })
end

return M
