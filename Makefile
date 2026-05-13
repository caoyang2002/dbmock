.PHONY: help db-json env run

BIN :=  ./target/release/datamocker
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

build:
	@echo "构建..."
	@cargo build --release

test:
	@echo "测试..."
	@$(BIN)

help:
	@echo "Available targets:"
	@echo " extract: 提取数据库结构"
	@echo " 	db-json: 提取数据库结构为 json 文件"
	@echo " 	db-sql: 提取数据库结构为 sql 文件"
	@echo " 	db-config: 生成数据库结构的配置文件"
	@echo " generate: 生成 mock 数据"
	@echo " 	db-json: 生成 mock 数据"


