
let host = $"felixb@($env.FERMI_IP)"
let migration = ls crates/simple-core/migrations/*.sql | sort-by modified | last


print "Removing existing database..."
let cmd = "sudo -u postgres psql -c \"DROP DATABASE IF EXISTS subroutine\""
ssh $host $cmd

print "Creating new database..."
let cmd = "sudo -u postgres psql -c \"CREATE DATABASE subroutine OWNER subroutine\""
ssh $host $cmd

print "Running migration..."
let cmd = $"sudo -u postgres psql -d subroutine"
open $migration.name | ssh $host $cmd
