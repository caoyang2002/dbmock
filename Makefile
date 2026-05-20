.PHONY: help build log test clear test-db init gen env run

BIN :=  ./target/release/dbmock
# 如果 .env 比 env.mk 新，则重新生成 env.mk
env.mk: .env
	@echo "# Auto-generated from .env" > $@
	@sed -E 's/^([^#=]+)=(.*)/export \1 := \2/' .env | grep -v '^#' >> $@

# 包含生成的变量定义
-include env.mk

# 将所有 make 变量导出为 shell 环境变量
export

# 现在环境变量已经默认加载，直接使用即可
env:
	@echo "DB_HOST=$$DB_HOST"
	@echo "DB_PORT=$$DB_PORT"
	@echo "DB_URL=$$DB_URL"

build:
	@echo "构建..."
	@cargo build --release
	@cp ./target/release/dbmock ./dbmock

log:
	@RUST_LOG=sqlx::query=debug ./dbmock extract -j schema.json 2> sql.log

test:
	@echo "测试..."
	@$(BIN) --version
clear:
	@rm -f schema.json
	@rm -f schema.sql
	@rm -f env.mk
	@rm -f mock_config.yml

# 测试数据库连接及所有表的权限
check:
	@echo "Testing connection to database..."
	@psql "$(DB_URL)" -c "SELECT 1" > /dev/null 2>&1 && \
		echo "✅ Database connection successful" || \
		(echo "❌ Failed to connect to database" && exit 1)
	@echo "Checking tables and permissions..."
	@psql "$(DB_URL)" -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname = 'public';" > /tmp/tables_$$.txt 2>/dev/null; \
	if [ ! -s /tmp/tables_$$.txt ]; then \
		echo "⚠️  No tables found in public schema. Please run: psql '$(DB_URL)' -f schema.sql"; \
		rm -f /tmp/tables_$$.txt; \
		exit 1; \
	fi; \
	FAILED=0; \
	while read -r tbl; do \
		if psql "$(DB_URL)" -c "SELECT 1 FROM \"$$tbl\" LIMIT 1" > /dev/null 2>&1; then \
			echo "  ✅ Read permission on table: $$tbl"; \
		else \
			echo "  ❌ No read permission on table: $$tbl (or table empty)"; \
			FAILED=1; \
		fi; \
	done < /tmp/tables_$$.txt; \
	rm -f /tmp/tables_$$.txt; \
	if [ $$FAILED -ne 0 ]; then exit 1; fi
	@echo "Checking sequence permissions..."
	@psql "$(DB_URL)" -t -A -c "SELECT sequence_name FROM information_schema.sequences WHERE sequence_schema = 'public';" > /tmp/seqs_$$.txt 2>/dev/null; \
	if [ -s /tmp/seqs_$$.txt ]; then \
		while read -r seq; do \
			if psql "$(DB_URL)" -c "SELECT nextval('\"$$seq\"')" > /dev/null 2>&1; then \
				echo "  ✅ USAGE permission on sequence: $$seq"; \
				psql "$(DB_URL)" -c "SELECT setval('\"$$seq\"', last_value) FROM \"$$seq\"" > /dev/null 2>&1; \
			else \
				echo "  ❌ No USAGE permission on sequence: $$seq"; \
				exit 1; \
			fi; \
		done < /tmp/seqs_$$.txt; \
		rm -f /tmp/seqs_$$.txt; \
	else \
		echo "  No sequences found."; \
	fi
	@echo "All permission checks passed."

init:
	@$(BIN) extract --db-url "$(DB_URL)"
	@$(BIN) config --force
gen:
	@$(BIN) generate --db-url "$(DB_URL)" -c mock_config.yml --count 1000
run: build init gen
	@echo "✅ 完成：构建、初始化、生成数据"

# 默认 100 万
PERF_ROWS ?= 1000000

## 运行性能测试：生成指定行数的用户数据并输出耗时与速率
perf:
	@chmod +x scripts/perf.sh
	@PERF_ROWS=$(PERF_ROWS) DB_URL="$(DB_URL)" BIN="$(BIN)" ./scripts/perf.sh

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  build       构建 release 版本并复制到 ./dbmock"
	@echo "  log         以 DEBUG 日志级别运行 extract 并记录 SQL 日志"
	@echo "  test        测试 dbmock 版本"
	@echo "  clear       清除生成的文件 (schema.json, schema.sql, env.mk, mock_config.yml, ./target)"
	@echo "  test-db     测试数据库连接、表权限和序列权限"
	@echo "  init        提取数据库 schema 并生成 mock 配置 (依赖 test-db)"
	@echo "  gen         生成模拟数据 (需要先运行 init)"
	@echo "  env         显示当前加载的环境变量 (DB_HOST, DB_PORT, DB_URL)"
	@echo "  run         依次执行 build, init, gen"
	@echo ""
	@echo "环境变量 (从 .env 文件加载):"
	@echo "  DB_HOST, DB_PORT, DB_URL"
	@echo ""
	@echo "示例:"
	@echo "  make init       # 提取 schema 并生成配置"
	@echo "  make gen        # 生成测试数据"
