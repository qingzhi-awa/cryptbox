package main

import (
	"os"
	"path/filepath"
)

// Config 服务端配置，通过环境变量注入。
type Config struct {
	Port      string // 监听端口
	DBDSN     string // SQLite 数据库文件路径
	JWTSecret string // JWT 签名密钥
}

// dataDir 返回数据目录：飞牛 FPK 运行时优先使用 TRIM_PKGVAR，否则当前目录。
func dataDir() string {
	if d := os.Getenv("TRIM_PKGVAR"); d != "" {
		return d
	}
	return "."
}

func loadConfig() Config {
	return Config{
		Port:      getEnv("PORT", "5201"),
		DBDSN:     getEnv("DB_DSN", filepath.Join(dataDir(), "passbook.db")),
		JWTSecret: getEnv("JWT_SECRET", "change-this-secret"),
	}
}

func getEnv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
