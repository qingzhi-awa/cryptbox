package main

import (
	"log"
	"net/http"
)

func main() {
	cfg := loadConfig()
	db, err := openDB(cfg)
	if err != nil {
		log.Fatalf("打开数据库失败: %v", err)
	}
	defer db.Close()

	encKey, err := getOrCreateEncryptionKey(db)
	if err != nil {
		log.Fatalf("初始化加密密钥失败: %v", err)
	}

	s := &Server{cfg: cfg, db: db, encKey: encKey}
	handler := serveWeb(s.routes())

	log.Printf("密匣服务已启动: http://localhost:%s (driver=sqlite)", cfg.Port)
	if err := http.ListenAndServe(":"+cfg.Port, handler); err != nil {
		log.Fatal(err)
	}
}
