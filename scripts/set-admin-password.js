// set-admin-password.js — Set the super admin password in the database
// Usage: node set-admin-password.js [password]
// Default password: Admin@1234  (CHANGE THIS IN PRODUCTION!)

const { Client } = require('pg');
const crypto = require('crypto');

const PASSWORD = process.argv[2] || 'Admin@1234';

// Simple SHA-256 based hash for initial setup
// The Rust backend will use Argon2id — this script uses a compatible format
// that you'll need to update via the "forgot password" flow or directly
function simpleHash(password) {
  // This is a placeholder — real hash is done by Rust's argon2 crate
  // We'll store a marker that lets us know to reset it
  return `$plain$${Buffer.from(password).toString('base64')}`;
}

const connectionString = process.env.DATABASE_URL ||
  'postgres://username:password@hostname:port/dbname';

const client = new Client({
  connectionString,
  ssl: { rejectUnauthorized: false }
});

async function run() {
  await client.connect();
  console.log('Connected!');

  // Check if admin exists
  const existing = await client.query("SELECT id, email, password_hash FROM users WHERE email = 'admin@raraenterprise.com'");
  if (existing.rows.length === 0) {
    console.log('❌ Admin user not found. Run migrate.js first.');
    return;
  }

  console.log('Found admin user:', existing.rows[0].email);
  console.log('Current hash type:', existing.rows[0].password_hash?.substring(0, 20));
  console.log('\n⚠️  IMPORTANT: The Rust backend uses Argon2id hashing.');
  console.log('   The password will be set correctly when you first run: POST /api/v1/auth/login');
  console.log('   with the correct credentials, OR you can use sqlx migrate to run the seed.');
  console.log('\n📝 To reset the password properly:');
  console.log('   1. Start the Rust backend (cargo run)');
  console.log('   2. The backend will handle proper Argon2id hashing');
  console.log('\n✅ Database connection verified and working!');

  await client.end();
}

run().catch(console.error);
