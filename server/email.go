package main

import (
	"crypto/tls"
	"database/sql"
	"errors"
	"fmt"
	"math/rand"
	"net/smtp"
	"strconv"
	"time"
)

// SMTPConfig 邮件配置。
type SMTPConfig struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	Username string `json:"username"`
	Password string `json:"password"`
	From     string `json:"from"`
	SSL      bool   `json:"ssl"`
	Vendor   string `json:"vendor"`
}

// loadSMTPConfig 从 meta 读取 SMTP 配置。
func loadSMTPConfig(db *sql.DB) SMTPConfig {
	port, _ := strconv.Atoi(getMeta(db, "smtp_port"))
	vendor := getMeta(db, "smtp_vendor")
	if vendor == "" {
		vendor = "qq"
	}
	return SMTPConfig{
		Host:     getMeta(db, "smtp_host"),
		Port:     port,
		Username: getMeta(db, "smtp_username"),
		Password: getMeta(db, "smtp_password"),
		From:     getMeta(db, "smtp_from"),
		SSL:      getMeta(db, "smtp_ssl") == "true",
		Vendor:   vendor,
	}
}

// emailVerifyMode 返回邮箱验证模式：code / link / none。
func emailVerifyMode(db *sql.DB) string {
	m := getMeta(db, "email_verify_mode")
	if m != "code" && m != "link" && m != "none" {
		return "none"
	}
	return m
}

// sendEmail 发送纯文本邮件。
func sendEmail(db *sql.DB, to, subject, body string) error {
	cfg := loadSMTPConfig(db)
	if cfg.Host == "" || cfg.Port == 0 {
		return errors.New("SMTP 未配置")
	}
	from := cfg.From
	if from == "" {
		from = cfg.Username
	}
	msg := []byte("From: " + from + "\r\n" +
		"To: " + to + "\r\n" +
		"Subject: " + subject + "\r\n" +
		"MIME-Version: 1.0\r\n" +
		"Content-Type: text/plain; charset=UTF-8\r\n" +
		"\r\n" + body)

	addr := cfg.Host + ":" + strconv.Itoa(cfg.Port)
	var auth smtp.Auth
	if cfg.Username != "" {
		auth = smtp.PlainAuth("", cfg.Username, cfg.Password, cfg.Host)
	}
	if cfg.SSL {
		conn, err := tls.Dial("tcp", addr, &tls.Config{ServerName: cfg.Host})
		if err != nil {
			return err
		}
		c, err := smtp.NewClient(conn, cfg.Host)
		if err != nil {
			return err
		}
		defer c.Close()
		if auth != nil {
			if err := c.Auth(auth); err != nil {
				return err
			}
		}
		if err := c.Mail(from); err != nil {
			return err
		}
		if err := c.Rcpt(to); err != nil {
			return err
		}
		w, err := c.Data()
		if err != nil {
			return err
		}
		if _, err := w.Write(msg); err != nil {
			return err
		}
		if err := w.Close(); err != nil {
			return err
		}
		return c.Quit()
	}
	return smtp.SendMail(addr, auth, from, []string{to}, msg)
}

// genCode 生成 6 位数字验证码。
func genCode() string {
	return fmt.Sprintf("%06d", rand.Intn(1000000))
}

// saveVerification 保存邮箱验证码（15 分钟有效，expires_at 存 Unix 时间戳）。
func saveVerification(db *sql.DB, email, code, purpose string) error {
	expires := strconv.FormatInt(time.Now().Add(15*time.Minute).Unix(), 10)
	_, err := db.Exec(`INSERT INTO email_verifications (email, code, purpose, expires_at) VALUES (?, ?, ?, ?)`,
		email, code, purpose, expires)
	return err
}

// checkVerification 校验邮箱验证码。
func checkVerification(db *sql.DB, email, code, purpose string) bool {
	now := strconv.FormatInt(time.Now().Unix(), 10)
	var n int
	err := db.QueryRow(`SELECT COUNT(1) FROM email_verifications WHERE email = ? AND code = ? AND purpose = ? AND expires_at > ?`,
		email, code, purpose, now).Scan(&n)
	return err == nil && n > 0
}

// sendVerifyCode 生成并发送验证码。
func sendVerifyCode(db *sql.DB, email, purpose string) error {
	code := genCode()
	if err := saveVerification(db, email, code, purpose); err != nil {
		return err
	}
	subject := "密匣验证码"
	body := "您的验证码是：" + code + "，15 分钟内有效。"
	if purpose == "reset" {
		subject = "密匣密码重置验证码"
		body = "您的密码重置验证码是：" + code + "，15 分钟内有效。"
	}
	return sendEmail(db, email, subject, body)
}
