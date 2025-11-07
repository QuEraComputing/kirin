#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Node<T: Copy + PartialEq> {
    info: T,
    next: Option<T>,
    prev: Option<T>,
}

trait UnwrapNode<T: Copy + PartialEq> {
    type Output;
    fn unwrap_node(self) -> Self::Output;
}

impl<'a, T: Copy + PartialEq> UnwrapNode<T> for Option<&'a Node<T>> {
    type Output = Option<&'a T>;
    /// Unwraps an Option containing a reference to a Node,
    /// returning an Option containing a reference to the Node's info.
    fn unwrap_node(self) -> Self::Output {
        match self {
            Some(node) => Some(&node.info),
            None => None,
        }
    }
}

impl<T: Copy + PartialEq> UnwrapNode<T> for Option<Node<T>> {
    type Output = Option<T>;
    /// Unwraps an Option containing a Node,
    /// returning an Option containing the Node's info.
    fn unwrap_node(self) -> Self::Output {
        match self {
            Some(node) => Some(node.info),
            None => None,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LinkedList<T: Copy + PartialEq> {
    head: Option<Node<T>>,
    tail: Option<Node<T>>,
    nodes: Vec<Node<T>>,
}

impl<T: Copy + PartialEq> LinkedList<T> {
    pub fn new() -> Self {
        LinkedList {
            head: None,
            tail: None,
            nodes: Vec::new(),
        }
    }

    pub fn head(&self) -> Option<&T> {
        self.head.as_ref().unwrap_node()
    }

    pub fn tail(&self) -> Option<&T> {
        self.tail.as_ref().unwrap_node()
    }

    /// Appends an element to the back of the linked list.
    pub fn push_back(&mut self, info: T) {
        let new_node = Node {
            info,
            next: None,
            prev: self.tail.clone().unwrap_node(),
        };

        if let Some(tail) = &mut self.tail {
            tail.next = Some(info);
        } else {
            self.head = Some(new_node.clone());
        }

        self.tail = Some(new_node.clone());
        self.nodes.push(new_node);
    }

    /// Returns an iterator over the elements of the linked list.
    pub fn iter(&self) -> LinkedListIterator<T> {
        LinkedListIterator {
            list: self,
            current: self.head.as_ref().map(|node| node.info),
        }
    }
}

pub struct LinkedListIterator<'a, T: Copy + PartialEq> {
    list: &'a LinkedList<T>,
    current: Option<T>,
}

impl<'a, T: Copy + PartialEq> Iterator for LinkedListIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_info = self.current?;
        let current_node = self
            .list
            .nodes
            .iter()
            .find(|node| node.info == current_info)?;

        self.current = current_node.next;
        Some(current_info)
    }
}
