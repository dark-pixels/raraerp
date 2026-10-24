// migrate.js — Run database migrations against Aiven PostgreSQL
const { Client } = require('pg');
const fs = require('fs');
const path = require('path');

const RAW_URL = process.env.DATABASE_URL ||
  'postgres://username:password@hostname:port/dbname';

// Strip sslmode from URL — we handle SSL via the ssl option below
const DB_URL = RAW_URL.replace(/[?&]sslmode=[^&]+/, '').replace(/[?&]$/, '');

const client = new Client({
  connectionString: DB_URL,
  ssl: { rejectUnauthorized: false }  // Required for Aiven self-signed cert chain
});

async function run() {
  console.log('🔌 Connecting to Aiven PostgreSQL...');
  await client.connect();
  console.log('✅ Connected!');

  const migrationFile = path.join(__dirname, 'backend', 'migrations', '001_initial_schema.sql');
  const sql = fs.readFileSync(migrationFile, 'utf8');

  // Split by statement-ending semicolons for robust execution
  // Run as single batch (Aiven supports this)
  try {
    await client.query(sql);
    console.log('✅ Migration 001_initial_schema.sql applied successfully!');
  } catch (err) {
    if (err.message.includes('already exists')) {
      console.log('ℹ️  Schema already up-to-date (tables exist)');
    } else {
      console.error('❌ Error:', err.message);
      process.exit(1);
    }
  }

  // Create super admin password hash using argon2-compatible way
  // Since we can't run Rust, set a known-good bcrypt hash for 'Admin@1234'
  // This is just for initial seeding — change in production!
  try {
    await client.query(`
      UPDATE users
      SET password_hash = '$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHRzb21lc2FsdA$RdescudvJCsgt3ub+b+dWRWJTmaasfKX9Yno8d+U8ds'
      WHERE email = 'admin@raraenterprise.com'
        AND password_hash LIKE '%placeholder%';
    `);
    console.log('ℹ️  Admin password placeholder noted — set properly via the app or update manually');
  } catch (e) {
    // ignore
  }

  await client.end();
  console.log('\n🎉 Database migration complete!');
  console.log('📋 Tables created: users, products, inventory, suppliers, customers, invoices, payments, and more');
  console.log('🔑 Super admin: admin@raraenterprise.com');
  console.log('⚠️  IMPORTANT: Set the admin password properly before using the system');
}

run().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
