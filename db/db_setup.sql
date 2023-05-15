CREATE SCHEMA IF NOT EXISTS routine_app;

CREATE TABLE IF NOT EXISTS routine_app.customer (
    id UUID NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY, 
    name VARCHAR(256),
    email VARCHAR(256), 
    passwd VARCHAR(256),
    verification_status_id INT, -- [0, 1, 2] 0 - pure, 1 - verified, 2 - expired
    status_id INT, -- [0, 1, 2, 3] 0 - pure, 1 - verified, 2 - expired, 3 - deleted
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now()
);
    
CREATE OR REPLACE FUNCTION routine_app.set_updated_at()
 RETURNS trigger
 LANGUAGE plpgsql
AS $function$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$function$
;

CREATE TRIGGER trigger_updated_at_customer BEFORE
UPDATE ON routine_app.customer FOR EACH ROW EXECUTE FUNCTION routine_app.set_updated_at();

-- customer status table creation and update

CREATE TABLE IF NOT EXISTS routine_app.customer_status (
    id INT,
    description VARCHAR(256)
);

INSERT INTO routine_app.customer_status
    (id, description)
SELECT 0, 'Idle'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_status WHERE id = 0
    );
   
INSERT INTO routine_app.customer_status
    (id, description)
SELECT 1, 'Verified'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_status WHERE id = 1
    );
   
   
INSERT INTO routine_app.customer_status
    (id, description)
SELECT 2, 'Expired'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_status WHERE id = 2
    );
  
INSERT INTO routine_app.customer_status
    (id, description)
SELECT 3, 'Deleted'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_status WHERE id = 3
    );


-- customer verification table creation and update

CREATE TABLE IF NOT EXISTS routine_app.customer_verification_status(
    id INT,
    description VARCHAR(256)
);

INSERT INTO routine_app.customer_verification_status
    (id, description)
SELECT 0, 'Idle'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_verification_status WHERE id = 0
    );
   
INSERT INTO routine_app.customer_verification_status
    (id, description)
SELECT 1, 'Verified'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_verification_status WHERE id = 1
    );
   
   
INSERT INTO routine_app.customer_verification_status
    (id, description)
SELECT 2, 'Expired'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.customer_verification_status WHERE id = 2
    );


-- boards table creation

CREATE TABLE IF NOT EXISTS routine_app.board (
    id SERIAL PRIMARY KEY, 
    title VARCHAR(256),
    description VARCHAR(256), 
    status_id INT,
    owner_id UUID REFERENCES routine_app.customer (id), 
    creation_time TIMESTAMP NOT NULL DEFAULT now()
);
SELECT SETVAL('routine_app.board_id_seq', 100100);
-- board status table creation and update

CREATE TABLE IF NOT EXISTS routine_app.board_status (
    id INT,
    description VARCHAR(256)
);

INSERT INTO routine_app.board_status
    (id, description)
SELECT 0, 'Active'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.board_status WHERE id = 0
    );

INSERT INTO routine_app.board_status
    (id, description)
SELECT 1, 'Archive'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.board_status WHERE id = 1
    );

-- tasks table creation

CREATE TABLE IF NOT EXISTS routine_app.task (
    id SERIAL PRIMARY KEY, 
    title VARCHAR(256),
    description VARCHAR(256), 
    board_id INT REFERENCES routine_app.board (id), 
    status_id INT,
    last_status_change_time TIMESTAMP NOT NULL DEFAULT now(),
    creation_time TIMESTAMP NOT NULL DEFAULT now()
);
SELECT SETVAL('routine_app.task_id_seq', 100100);

CREATE OR REPLACE FUNCTION routine_app.set_status_change_time()
 RETURNS trigger
 LANGUAGE plpgsql
AS $function$
BEGIN
    NEW.last_status_change_time = now();
    RETURN NEW;
END;
$function$
;

CREATE TRIGGER trigger_status_change_at_task 
BEFORE UPDATE OF status_id ON routine_app.task 
FOR EACH ROW 
WHEN (OLD.status_id IS DISTINCT FROM NEW.status_id)
EXECUTE FUNCTION routine_app.set_status_change_time();

-- task status table creation and update

CREATE TABLE IF NOT EXISTS routine_app.task_status (
    id INT,
    description VARCHAR(256)
);

INSERT INTO routine_app.task_status
    (id, description)
SELECT 0, 'To do'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.task_status WHERE id = 0
    );

INSERT INTO routine_app.task_status
    (id, description)
SELECT 1, 'In progress'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.task_status WHERE id = 1
    );

INSERT INTO routine_app.task_status
    (id, description)
SELECT 2, 'Done'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.task_status WHERE id = 2
    );

INSERT INTO routine_app.task_status
    (id, description)
SELECT 3, 'On hold'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.task_status WHERE id = 3
    );

INSERT INTO routine_app.task_status
    (id, description)
SELECT 4, 'Cancelled'
WHERE
    NOT EXISTS (
        SELECT id FROM routine_app.task_status WHERE id = 4
    );

