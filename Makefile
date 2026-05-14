.PHONY: help db-json env run

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
run: $(BIN)
	$(BIN)
db-json:
	@echo "开始提取数据库结构..."

db-sql:
	@echo "开始提取数据库结构..."
	$(BIN) extract -s schema.sql

build:
	@echo "构建..."
	@cargo build --release
	@cp ./target/release/dbmock ./dbmock

log:
	@RUST_LOG=sqlx::query=debug ./dbmock extract -j schema.json 2> sql.log

test:
	@echo "测试..."
	@$(BIN)
clear:
	@rm -f schema.json
	@rm -f schema.sql
	@rm -f env.mk
	@rm -f mock_config.yml

# 测试数据库连接及所有表的权限
test-db:
	@echo "Testing connection to database..."
	@psql "$(DATABASE_URL)" -c "SELECT 1" > /dev/null 2>&1 && \
		echo "✅ Database connection successful" || \
		(echo "❌ Failed to connect to database" && exit 1)
	@echo "Checking tables and permissions..."
	@psql "$(DATABASE_URL)" -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname = 'public';" > /tmp/tables_$$.txt 2>/dev/null; \
	if [ ! -s /tmp/tables_$$.txt ]; then \
		echo "⚠️  No tables found in public schema. Please run: psql '$(DATABASE_URL)' -f schema.sql"; \
		rm -f /tmp/tables_$$.txt; \
		exit 1; \
	fi; \
	FAILED=0; \
	while read -r tbl; do \
		if psql "$(DATABASE_URL)" -c "SELECT 1 FROM \"$$tbl\" LIMIT 1" > /dev/null 2>&1; then \
			echo "  ✅ Read permission on table: $$tbl"; \
		else \
			echo "  ❌ No read permission on table: $$tbl (or table empty)"; \
			FAILED=1; \
		fi; \
	done < /tmp/tables_$$.txt; \
	rm -f /tmp/tables_$$.txt; \
	if [ $$FAILED -ne 0 ]; then exit 1; fi
	@echo "Checking sequence permissions..."
	@psql "$(DATABASE_URL)" -t -A -c "SELECT sequence_name FROM information_schema.sequences WHERE sequence_schema = 'public';" > /tmp/seqs_$$.txt 2>/dev/null; \
	if [ -s /tmp/seqs_$$.txt ]; then \
		while read -r seq; do \
			if psql "$(DATABASE_URL)" -c "SELECT nextval('\"$$seq\"')" > /dev/null 2>&1; then \
				echo "  ✅ USAGE permission on sequence: $$seq"; \
				psql "$(DATABASE_URL)" -c "SELECT setval('\"$$seq\"', last_value) FROM \"$$seq\"" > /dev/null 2>&1; \
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

init: test-db
	@$(BIN) extract --database-url "$(DATABASE_URL)"
	@$(BIN) config --force
gen:
	# @$(BIN) generate --database-url "$(DATABASE_URL)"




help:
	@echo "Available targets:"
	@echo " extract: 提取数据库结构"
	@echo " 	db-json: 提取数据库结构为 json 文件"
	@echo " 	db-sql: 提取数据库结构为 sql 文件"
	@echo " 	db-config: 生成数据库结构的配置文件"
	@echo " generate: 生成 mock 数据"
	@echo " 	db-json: 生成 mock 数据"
