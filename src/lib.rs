use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        println!("Sending terminate message to all workers.");

        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        println!("Shutting down all workers.");

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job) => {
                    println!("Worker {} got a job; executing.", id);

                    job();
                }
                Message::Terminate => {
                    println!("Worker {} was told to terminate.", id);

                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::time::Duration;

// pyO3 module
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::wrap_pyfunction;

use std::future::Future;

#[pyclass]
struct Server {}

#[pymethods]
impl Server {
    #[new]
    fn new() -> Self {
        Self {}
    }

    fn start(mut self_: PyRefMut<Self>, test: &PyAny) {
        // let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
        // let pool = ThreadPool::new(4);

        test.call0();

        // for stream in listener.incoming() {
        //     let stream = stream.unwrap();

        //     pool.execute(|| {
        //         let rt = tokio::runtime::Runtime::new().unwrap();
        //         let mut contents = String::new();
        //         handle_connection(stream, rt, &mut contents, &test_helper);
        //     });
        // }
    }
}

#[pyfunction]
pub fn start_server() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4);

    // test()

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let mut contents = String::new();
            handle_connection(stream, rt, &mut contents, &test_helper);
        });
    }
}

#[pymodule]
pub fn roadrunner(_: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(start_server))?;
    m.add_class::<Server>()?;
    Ok(())
}

async fn read_file(filename: String) -> String {
    let con = tokio::fs::read_to_string(filename).await;
    con.unwrap()
}

async fn test_helper(
    contents: &mut String,
    filename: String,
    status_line: String,
    mut stream: TcpStream,
) {
    // this function will accept custom function and return
    *contents = tokio::task::spawn(read_file(filename.clone()))
        .await
        .unwrap();

    let len = contents.len();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line, len, contents
    );

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
    // return String::from(contents.clone());
}

// let mut contents = String::new();

pub fn handle_connection<'a, F>(
    mut stream: TcpStream,
    runtime: tokio::runtime::Runtime,
    contents: &'a mut String,
    test: &dyn Fn(&'a mut String, String, String, TcpStream) -> F,
) where
    F: Future<Output = ()> + 'a,
{
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let get = b"GET / HTTP/1.1\r\n";
    let sleep = b"GET /sleep HTTP/1.1\r\n";

    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if buffer.starts_with(sleep) {
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let future = test(
        contents,
        String::from(filename),
        String::from(status_line),
        stream,
    );
    runtime.block_on(future);
}
