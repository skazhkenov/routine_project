const express = require('express');

const app = express();
const ejs = require('ejs');

//app.use(express.static(__dirname));
console.log(__dirname + "");
app.set('views',__dirname + '/views');

app.set('view engine', 'ejs');

app.use(express.static('public'));

app.use(express.static(__dirname + '/public'));

app.get('/board/:board_id', (req, res) => {

	  const boardId = req.params.board_id;
	  const data = {
		"board_id": boardId
	  };
	  res.render('board', data);
});

app.get('/show_task/:board_id/:task_id', (req, res) => {

	const boardId = req.params.board_id;
	const taskId = req.params.task_id;
	const data = {
	  "board_id": boardId, 
	  "task_id": taskId
	};
	res.render('updateable_task', data);
});

app.get('/account', (req, res) => {

	res.render('account');
});


app.get('/boards', (req, res) => {

	  res.render('workspace');
});


app.listen(3000, () => {
	  console.log('Server started on port 3000');
});
